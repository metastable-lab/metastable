pub trait EnvVars {
    fn load() -> Self;
    fn get_env_var(&self, key: &str) -> String;
}