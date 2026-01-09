use std::sync::Arc;

use ::minijinja::Environment;

pub struct Renderer {
    env: Arc<Environment<'static>>,
    template_name: String,
}

impl Renderer {
    pub fn new(env: Arc<Environment<'static>>, template_name: impl Into<String>) -> Self {
        Self {
            env,
            template_name: template_name.into(),
        }
    }
}

impl crate::ssr::Renderer for Renderer {
    fn render(&self, data: crate::ssr::Data) -> Result<String, crate::ssr::Error> {
        self.env
            .get_template(&self.template_name)
            .map_err(|e| crate::ssr::Error(Box::new(e)))?
            .render(data)
            .map_err(|e| crate::ssr::Error(Box::new(e)))
    }
}
