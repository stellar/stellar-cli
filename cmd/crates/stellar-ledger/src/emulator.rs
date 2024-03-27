use crate::docker::DockerConnection;

pub enum Error {}

pub struct Emulator {
    docker: DockerConnection,
    container_id: Option<String>,
}

impl Emulator {
    pub async fn new() -> Self {
        let d = DockerConnection::new().await;

        Self {
            docker: d,
            container_id: None,
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        let zondax_speculos_image =
            "docker.io/zondax/builder-zemu:speculos-3a3439f6b45eca7f56395673caaf434c202e7005";
        self.docker
            .get_image_with_defaults(zondax_speculos_image)
            .await
            .unwrap();

        let container_id = self
            .docker
            .get_container_with_defaults(zondax_speculos_image)
            .await
            .unwrap();

        self.container_id = Some(container_id.clone());

        // This is starting up, but i think it fails pretty quickly, and i think we have it configured to delete itself once it starts. yep, when auto_remove is set to false, it sticks around but it exits right away
        self.docker
            .start_container_with_defaults(&container_id)
            .await
            .unwrap();

        // self.docker.stream_logs(&container_id).await;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Error> {
        if let Some(container_id) = &self.container_id {
            self.docker.stop_container(container_id).await;
        }
        Ok(())
    }
}

// -------------------------------------------------------------

// next steps:
// have this docker connection start the speculos emulator
// see if i can use that emulator in tests like they do with zemu
