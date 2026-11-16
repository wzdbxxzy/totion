mod api;
mod auth;
mod db;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
#[command(name = "totion-server", version, about = "Totion 共享空间同步服务")]
struct Args {
    /// 监听地址
    #[arg(long, env = "TOTION_BIND", default_value = "0.0.0.0:8080")]
    bind: String,

    /// 数据目录（SQLite 与邀请码）
    #[arg(long, env = "TOTION_DATA_DIR", default_value = "data")]
    data_dir: PathBuf,

    /// 固定邀请码（不设置则随机生成并写入数据目录）
    #[arg(long, env = "TOTION_INVITE_CODE")]
    invite_code: Option<String>,

    /// 打印当前邀请码后退出
    #[arg(long)]
    print_invite: bool,

    /// 生成一个新邀请码并退出；参数是备注
    #[arg(long, value_name = "NOTE")]
    new_invite: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    std::fs::create_dir_all(&args.data_dir)?;

    let conn = db::open(&args.data_dir.join("totion.db"))?;
    let invite = auth::init_invite(&conn, args.invite_code.as_deref())?;
    let invite_path = args.data_dir.join("invite_code");
    std::fs::write(&invite_path, format!("{invite}\n"))?;

    if args.print_invite {
        println!("{invite}");
        return Ok(());
    }

    if let Some(note) = args.new_invite.as_deref() {
        let now = chrono::Utc::now();
        for _ in 0..8 {
            let candidate = totion_core::Invite {
                id: uuid::Uuid::new_v4().to_string(),
                code: auth::generate_invite_code(),
                note: note.to_string(),
                created_by: "server".to_string(),
                created_at: now,
                expires_at: None,
                max_uses: None,
                used_count: 0,
                revoked: false,
            };
            match db::insert_invite(&conn, &candidate) {
                Ok(()) => {
                    println!("{}", candidate.code);
                    return Ok(());
                }
                Err(err) if err.to_string().contains("UNIQUE") => continue,
                Err(err) => return Err(err.into()),
            }
        }
        anyhow::bail!("生成邀请码失败，请重试");
    }

    let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
    if users == 0 {
        tracing::info!(
            "还没有用户。邀请码: {invite}（已写入 {}）",
            invite_path.display()
        );
    }

    let db = Arc::new(Mutex::new(conn));
    let app = api::router(db);

    let listener = TcpListener::bind(&args.bind).await?;
    tracing::info!("totion-server 监听 http://{}", args.bind);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("安装 SIGTERM 处理器失败")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("收到退出信号，正在关闭");
}
