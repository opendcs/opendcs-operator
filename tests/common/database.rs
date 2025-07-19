#[cfg(test)]
pub mod tests {
    use kube::Client;

    pub struct PostgresCredentials {
        pub secret_name: String
    }

    pub async fn create_postgres_instance(client: Client) -> anyhow::Result<PostgresCredentials> {

        Ok(PostgresCredentials { secret_name: "tmp".into() })
    }
}