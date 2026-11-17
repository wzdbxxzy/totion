//! 网络线程：登录、增量同步、写操作都在这儿跑，UI 只收发消息。

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Duration;

use chrono::Local;
use totion_core::{
    BlockKind, CheckinRequest, CreateBlockRequest, CreateCommentRequest, CreateHabitRequest,
    CreateInviteRequest, CreateJournalRequest, Invite, RegisterRequest, UpdateBlockRequest,
    UpdateCommentRequest, UpdateHabitRequest, UpdateJournalRequest, VerifyRequest,
};

use crate::api::{Api, ApiError};
use crate::identity::Identity;
use crate::store::Store;

pub enum Command {
    CreateBlock {
        page_id: String,
        kind: BlockKind,
        text: String,
        due_date: Option<String>,
        start_time: Option<String>,
    },
    UpdateBlock {
        id: String,
        version: i64,
        text: Option<String>,
        checked: Option<bool>,
        deleted: bool,
    },
    EnsureDay {
        date: String,
    },
    /// 在指定日期下新建待办：先确保当天页面存在，再写块。
    CreateBlockOnDate {
        date: String,
        text: String,
        start_time: Option<String>,
    },
    CheckIn {
        habit_id: String,
        date: String,
    },
    UndoCheckIn {
        id: String,
        version: i64,
    },
    CreateHabit {
        title: String,
        icon: String,
        frequency: totion_core::HabitFrequency,
    },
    UpdateHabit {
        id: String,
        version: i64,
        title: Option<String>,
        archived: Option<bool>,
        deleted: bool,
    },
    CreateJournal {
        target_kind: totion_core::JournalTarget,
        target_id: String,
        body: String,
    },
    UpdateJournal {
        id: String,
        version: i64,
        body: Option<String>,
        deleted: bool,
    },
    CreateComment {
        target_kind: totion_core::CommentTargetKind,
        target_id: String,
        body: String,
    },
    UpdateComment {
        id: String,
        version: i64,
        body: Option<String>,
        resolved: Option<bool>,
        deleted: bool,
    },
    ListInvites,
    CreateInvite {
        note: String,
    },
    CreatePage {
        title: String,
    },
    UpdatePage {
        id: String,
        version: i64,
        title: Option<String>,
        deleted: bool,
    },
    Members,
    SyncNow,
}

pub enum Event {
    Ready {
        me: String,
        user_id: String,
        store: Box<Store>,
    },
    Changes {
        changes: Vec<totion_core::Change>,
        last_seq: i64,
    },
    Ok(String),
    Error(String),
    Invites(Vec<Invite>),
    Members(Vec<totion_core::User>),
}

pub struct Net {
    pub tx: Sender<Command>,
    pub rx: Receiver<Event>,
    /// 由 UI 线程在网络线程启动前塞进去，避免两套通道。
    pub start_tx: Sender<Start>,
}

pub struct Start {
    pub identity: Identity,
    pub invite_code: Option<String>,
}

pub fn spawn() -> Net {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<Command>();
    let (evt_tx, evt_rx) = std::sync::mpsc::channel::<Event>();
    let (start_tx, start_rx) = std::sync::mpsc::channel::<Start>();
    std::thread::spawn(move || worker(start_rx, cmd_rx, evt_tx));
    Net {
        tx: cmd_tx,
        rx: evt_rx,
        start_tx,
    }
}

fn worker(start_rx: Receiver<Start>, cmd_rx: Receiver<Command>, tx: Sender<Event>) {
    let Ok(start) = start_rx.recv() else {
        return;
    };
    let mut api = Api::new(&start.identity.server_url);
    let auth = match &start.invite_code {
        Some(code) => api.register(&RegisterRequest {
            invite_code: code.clone(),
            name: start.identity.name.clone(),
            public_key: start.identity.public_key.clone(),
        }),
        None => login(&api, &start.identity),
    };
    let auth = match auth {
        Ok(auth) => auth,
        Err(err) => {
            let _ = tx.send(Event::Error(format!("登录失败：{err}")));
            return;
        }
    };
    api.set_token(Some(auth.token.clone()));
    let mut saved = start.identity.clone();
    saved.user_id = auth.user.id.clone();
    saved.token = Some(auth.token);
    let _ = crate::identity::save_identity(&saved);

    let snapshot = match api.snapshot() {
        Ok(snapshot) => snapshot,
        Err(err) => {
            let _ = tx.send(Event::Error(format!("拉取数据失败：{err}")));
            return;
        }
    };
    let store = Store::from_snapshot(snapshot);
    let me = auth.user.name.clone();
    let user_id = auth.user.id.clone();
    let mut last_seq = store.last_seq;
    if tx
        .send(Event::Ready {
            me,
            user_id,
            store: Box::new(store),
        })
        .is_err()
    {
        return;
    }

    loop {
        match cmd_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Command::SyncNow) => {
                if let Some(seq) = sync(&api, last_seq, &tx) {
                    last_seq = seq;
                }
            }
            Ok(command) => {
                run_command(&api, command, &tx);
                // 写完立刻同步一次，把服务端版本拉回来，避免下一次点击用到过期版本。
                if let Some(seq) = sync(&api, last_seq, &tx) {
                    last_seq = seq;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Some(seq) = sync(&api, last_seq, &tx) {
                    last_seq = seq;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // 排空积压的点击，避免连续写操作排队太久。
        loop {
            match cmd_rx.try_recv() {
                Ok(command) => {
                    run_command(&api, command, &tx);
                    if let Some(seq) = sync(&api, last_seq, &tx) {
                        last_seq = seq;
                    }
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }
}

fn login(api: &Api, identity: &Identity) -> Result<totion_core::AuthResponse, ApiError> {
    let challenge = api.challenge(&identity.public_key)?;
    let signature = identity
        .sign(challenge.nonce.as_bytes())
        .map_err(|err| ApiError::Other(format!("签名失败: {err}")))?;
    api.verify(&VerifyRequest {
        challenge_id: challenge.challenge_id,
        signature,
    })
}

fn sync(api: &Api, since: i64, tx: &Sender<Event>) -> Option<i64> {
    let mut cursor = since;
    for _ in 0..20 {
        match api.sync(cursor) {
            Ok(response) => {
                let last = response.last_seq;
                // 即使没有变更也发一次，UI 用它刷新「最后一次同步」心跳。
                let _ = tx.send(Event::Changes {
                    changes: response.changes,
                    last_seq: last,
                });
                cursor = last.max(cursor);
                if !response.has_more {
                    break;
                }
            }
            Err(err) => {
                let _ = tx.send(Event::Error(format!("同步失败：{err}")));
                break;
            }
        }
    }
    Some(cursor)
}

fn run_command(api: &Api, command: Command, tx: &Sender<Event>) -> Option<i64> {
    let result = match command {
        Command::CreateBlock {
            page_id,
            kind,
            text,
            due_date,
            start_time,
        } => api
            .create_block(&CreateBlockRequest {
                page_id,
                kind,
                text,
                due_date,
                start_time,
                end_time: None,
                position: 0,
                section: None,
            })
            .map(|_| "已创建".to_string()),
        Command::UpdateBlock {
            id,
            version,
            text,
            checked,
            deleted,
        } => api
            .update_block(
                &id,
                &UpdateBlockRequest {
                    expected_version: version,
                    due_date: None,
                    streak: None,
                    position: None,
                    text,
                    checked,
                    start_time: None,
                    end_time: None,
                    deleted: Some(deleted),
                },
            )
            .map(|_| "已保存".to_string()),
        Command::EnsureDay { date } => api.ensure_day(&date).map(|_| String::new()),
        Command::CreateBlockOnDate {
            date,
            text,
            start_time,
        } => match api.ensure_day(&date) {
            Ok(day) => api
                .create_block(&CreateBlockRequest {
                    page_id: day.page.id,
                    kind: BlockKind::Todo,
                    text,
                    due_date: Some(date),
                    position: 0,
                    start_time,
                    end_time: None,
                    section: None,
                })
                .map(|_| "已新建待办".to_string()),
            Err(err) => Err(err),
        },
        Command::CheckIn { habit_id, date } => api
            .check_in(&habit_id, &CheckinRequest { date, note: None })
            .map(|_| "已打卡".to_string()),
        Command::UndoCheckIn { id, version } => api
            .update_checkin(
                &id,
                &totion_core::UpdateCheckinRequest {
                    expected_version: version,
                    note: None,
                    deleted: Some(true),
                },
            )
            .map(|_| "已撤销".to_string()),
        Command::CreateHabit {
            title,
            icon,
            frequency,
        } => api
            .create_habit(&CreateHabitRequest {
                title,
                icon,
                note: String::new(),
                frequency,
                position: 0,
            })
            .map(|_| "已创建习惯".to_string()),
        Command::UpdateHabit {
            id,
            version,
            title,
            archived,
            deleted,
        } => api
            .update_habit(
                &id,
                &UpdateHabitRequest {
                    expected_version: version,
                    title,
                    icon: None,
                    note: None,
                    frequency: None,
                    position: None,
                    archived,
                    deleted: Some(deleted),
                },
            )
            .map(|_| "已保存".to_string()),
        Command::CreateJournal {
            target_kind,
            target_id,
            body,
        } => {
            let now = Local::now().naive_local();
            api.create_journal(&CreateJournalRequest {
                target_kind,
                target_id,
                date: now.format("%Y-%m-%d").to_string(),
                time: now.format("%H:%M").to_string(),
                body,
            })
            .map(|_| "已记录".to_string())
        }
        Command::UpdateJournal {
            id,
            version,
            body,
            deleted,
        } => api
            .update_journal(
                &id,
                &UpdateJournalRequest {
                    expected_version: version,
                    body,
                    deleted: Some(deleted),
                },
            )
            .map(|_| "已保存".to_string()),
        Command::CreateComment {
            target_kind,
            target_id,
            body,
        } => api
            .create_comment(&CreateCommentRequest {
                target_kind,
                target_id,
                body,
            })
            .map(|_| "已留言".to_string()),
        Command::UpdateComment {
            id,
            version,
            body,
            resolved,
            deleted,
        } => api
            .update_comment(
                &id,
                &UpdateCommentRequest {
                    expected_version: version,
                    body,
                    resolved,
                    deleted: Some(deleted),
                },
            )
            .map(|_| "已更新".to_string()),
        Command::ListInvites => match api.list_invites() {
            Ok(list) => {
                let _ = tx.send(Event::Invites(list.invites));
                return None;
            }
            Err(err) => Err(err),
        },
        Command::CreateInvite { note } => api
            .create_invite(&CreateInviteRequest {
                note,
                expires_in_days: None,
                max_uses: Some(1),
            })
            .map(|response| format!("新邀请码 {}", response.invite.code)),
        Command::CreatePage { title } => api
            .create_page(&totion_core::CreatePageRequest {
                title,
                icon: String::new(),
                kind: totion_core::PageKind::Doc,
                position: 0,
                date: None,
            })
            .map(|_| "已新建页面".to_string()),
        Command::UpdatePage {
            id,
            version,
            title,
            deleted,
        } => api
            .update_page(
                &id,
                &totion_core::UpdatePageRequest {
                    expected_version: version,
                    title,
                    icon: None,
                    kind: None,
                    position: None,
                    deleted: Some(deleted),
                    date: None,
                },
            )
            .map(|_| "已更新页面".to_string()),
        Command::Members => match api.members() {
            Ok(list) => {
                let _ = tx.send(Event::Members(list.users));
                return None;
            }
            Err(err) => Err(err),
        },
        Command::SyncNow => return None,
    };
    match result {
        Ok(message) => {
            let _ = tx.send(Event::Ok(message));
        }
        Err(err) => {
            let _ = tx.send(Event::Error(err.to_string()));
        }
    }
    None
}
