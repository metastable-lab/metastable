#[async_trait::async_trait]
pub trait ModuleClient: Clone + Send + Sync + 'static {
    const NAME: &'static str;
    type Client;

    fn validate_env() -> bool;
    async fn setup_connection() -> Self;

    fn get_client(&self) -> &Self::Client;
}

#[macro_export]
macro_rules! define_module_client {
    {
        (struct $struct_name:ident, $client_name:expr)
        client_type: $client_type:ty,
        env: [ $( $env_var:literal ),* ],
        setup: $setup_logic:expr
    } => {
        #[derive(Clone)]
        pub struct $struct_name {
            client: Option<std::sync::Arc<$client_type>>,
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self {
                    client: None,
                }
            }
        }

        #[async_trait::async_trait]
        impl ::metastable_common::ModuleClient for $struct_name {
            const NAME: &'static str = $client_name;
            type Client = std::sync::Arc<$client_type>;

            fn validate_env() -> bool {
                const ENV_VARS: &'static [&'static str] = &[ $( $env_var ),* ];
                let missing_vars: Vec<&'static str> = ENV_VARS.iter().cloned().filter(|var| std::env::var(var).is_err()).collect();

                if missing_vars.is_empty() {
                    return true;
                }

                let vars_str = missing_vars.join(", ");
                tracing::error!("[Client: {}] Required environment variables are not set: [{}]", $client_name, &vars_str);
                false
            }

            async fn setup_connection() -> Self {
                if !Self::validate_env() {
                    panic!("[Client: {}] Required environment variables are not set. Check logs for details. Cannot setup connection.", $client_name);
                }

                let client_instance = $setup_logic.await;
                Self {
                    client: Some(std::sync::Arc::new(client_instance)),
                }
            }

            fn get_client(&self) -> &Self::Client {
                self.client.as_ref().expect("Client not connected. Did you call setup_connection?")
            }
        }
    }
}
