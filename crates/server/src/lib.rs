pub mod health {
    use serde::Serialize;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "lowercase")]
    pub enum AtprotoStatus {
        Reachable,
        Unreachable,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct HealthReport {
        pub status: &'static str,
        pub atproto: AtprotoStatus,
    }

    pub fn health_report(_atproto: AtprotoStatus) -> HealthReport {
        todo!("not implemented")
    }
}
