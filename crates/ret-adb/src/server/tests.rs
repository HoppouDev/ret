use super::*;
use std::collections::HashSet;
use std::net::TcpListener;

fn state() -> State {
    State {
        managed: true,
        guards: 1,
        child: None,
    }
}

/// A free port that no other test of this run received
///
/// The probe listener is closed before the server fixture binds the port, so without this the
/// system may give the same port to a test running in parallel, which then sees the other
/// test's server as an existing one.
fn address() -> SocketAddr {
    static GIVEN: Mutex<Option<HashSet<u16>>> = Mutex::new(None);
    let mut given = GIVEN.lock().unwrap_or_else(|e| e.into_inner());
    let given = given.get_or_insert_with(HashSet::new);
    loop {
        let address = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap();
        if given.insert(address.port()) {
            return address;
        }
    }
}

fn command(address: SocketAddr) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "server::tests::server_process", "--ignored"])
        .env("RET_TEST_SERVER_ADDRESS", address.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[test]
#[ignore = "subprocess fixture launched by server lifecycle tests"]
fn server_process() {
    let listener = TcpListener::bind(std::env::var("RET_TEST_SERVER_ADDRESS").unwrap()).unwrap();
    for stream in listener.incoming() {
        drop(stream.unwrap());
    }
}

#[test]
fn owned_server_stops_and_releases_its_listener() {
    let address = address();
    let mut state = state();
    state.start_managed(&mut command(address), address).unwrap();
    assert!(connect(address).is_ok());
    drop(state.child.take());
    assert!(connect(address).is_err());
}

#[test]
fn reuses_existing_server_without_claiming_ownership() {
    let address = address();
    let mut external = state();
    external
        .start_managed(&mut command(address), address)
        .unwrap();
    let mut application = state();
    application
        .start_managed(&mut Command::new("must-not-be-launched"), address)
        .unwrap();
    assert!(application.child.is_none());
    drop(application);
    assert!(connect(address).is_ok());
    drop(external);
    assert!(connect(address).is_err());
}

#[test]
fn restarts_an_exited_owned_server() {
    let address = address();
    let mut state = state();
    state.start_managed(&mut command(address), address).unwrap();
    let child = state.child.as_mut().unwrap();
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    state.start_managed(&mut command(address), address).unwrap();
    assert!(connect(address).is_ok());
    drop(state);
    assert!(connect(address).is_err());
}

#[test]
fn background_work_cannot_restart_after_window_shutdown() {
    let address = address();
    let mut state = state();
    state.guards = 0;
    let error = state
        .start_managed(&mut command(address), address)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert!(connect(address).is_err());
}
