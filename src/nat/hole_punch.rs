use tokio::net::UdpSocket;

/* =========================
   HOLE PUNCH
========================= */

pub struct HolePunch {
    socket: UdpSocket,
}

impl HolePunch {
    pub async fn new(ip: &str, port: u16) -> anyhow::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(format!("{ip}:{port}")).await?
        })
    }

    pub async fn punch(&self, target_ip: &str, target_port: u16) -> anyhow::Result<()> {
        self.socket.send_to(&[1u8], format!("{target_ip}:{target_port}")).await?;
        Ok(())
    }
}

/* =========================
   TEST
========================= */

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hole_punch_send() {
        let receiver = UdpSocket::bind("127.0.0.1:47000").await.unwrap();
        let puncher = HolePunch::new("127.0.0.1", 47001).await.unwrap();

        puncher.punch("127.0.0.1", 47000).await.unwrap();

        let mut buf = [0u8; 1];
        let (size, _) = receiver.recv_from(&mut buf).await.unwrap();

        assert_eq!(size, 1);
        assert_eq!(buf[0], 1u8);
    }
}
