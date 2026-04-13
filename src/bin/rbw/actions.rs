use std::io::Read as _;

use anyhow::Context as _;

pub fn register() -> anyhow::Result<()> {
    simple_action(rbw::protocol::Action::Register)
}

pub fn quit() -> anyhow::Result<()> {
    match crate::sock::Sock::connect() {
        Ok(mut sock) => {
            let pidfile = rbw::dirs::pid_file();
            let mut pid = String::new();
            std::fs::File::open(pidfile)?.read_to_string(&mut pid)?;
            let Some(pid) =
                rustix::process::Pid::from_raw(pid.trim_end().parse()?)
            else {
                anyhow::bail!("failed to read pid from pidfile");
            };
            sock.send(&rbw::protocol::Request::new(
                get_environment(),
                rbw::protocol::Action::Quit,
            ))?;
            wait_for_exit(pid);
            Ok(())
        }
        Err(e) => match e.kind() {
            // if the socket doesn't exist, or the socket exists but nothing
            // is listening on it, the agent must already be not running
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(e.into()),
        },
    }
}

pub fn clipboard_store(text: &str) -> anyhow::Result<()> {
    simple_action(rbw::protocol::Action::ClipboardStore {
        text: text.to_string(),
    })
}

fn simple_action(action: rbw::protocol::Action) -> anyhow::Result<()> {
    let mut sock = connect()?;

    sock.send(&rbw::protocol::Request::new(get_environment(), action))?;

    let res = sock.recv()?;
    match res {
        rbw::protocol::Response::Ack => Ok(()),
        rbw::protocol::Response::Error { error } => {
            Err(anyhow::anyhow!("{error}"))
        }
        _ => Err(anyhow::anyhow!("unexpected message: {res:?}")),
    }
}

fn connect() -> anyhow::Result<crate::sock::Sock> {
    crate::sock::Sock::connect().with_context(|| {
        let log = rbw::dirs::agent_stderr_file();
        format!(
            "failed to connect to rbw-agent \
            (this often means that the agent failed to start; \
            check {} for agent logs)",
            log.display()
        )
    })
}

fn wait_for_exit(pid: rustix::process::Pid) {
    loop {
        if rustix::process::test_kill_process(pid).is_err() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub fn get_environment() -> rbw::protocol::Environment {
    rbw::protocol::Environment::from_current()
}
