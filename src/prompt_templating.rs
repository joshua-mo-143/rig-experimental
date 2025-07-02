use tera::Context;

pub struct PromptTemplate {
    template: String,
    variables: Context
}

impl PromptTemplate {
    pub fn from_string(str: &str) -> Self {
        Self {
            template: str.to_string(),
            variables: Context::new()
        }
    }

    pub fn from_file<P>(path: P) -> Self where P: AsRef<Path> {
        let str = std::fs::read_to_string(path).unwrap();

        Self {
            template: str.to_string(),
            variables: Context::new()
        }
    }

    pub fn with_variable(mut self, k: &str, v: &str) -> Self {
        self.variables.insert(k, v);
        self
    }

    pub fn set_variable(&mut self, k: &str, v: &str) {
        self.variables.insert(k, v);
    }

    pub fn render_to_string(&self) -> String {
        tera::Tera::one_off(&self.template, self.variables, false).unwrap()
    }
}