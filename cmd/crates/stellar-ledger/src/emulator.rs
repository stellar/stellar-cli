use crate::docker::DockerConnection;

pub enum Error {}

pub async fn run() -> Result<(), Error> {
    DockerConnection::new().await;
    Ok(())
}



// next steps:
// have this docker connection start the speculos emulator
// see if i can use that emulator in tests like they do with zemu
