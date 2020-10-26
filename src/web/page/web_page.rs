use crate::web::{
    error::{DocsrsError, DocsrsResult},
    TemplateData,
};
use serde::Serialize;
use std::borrow::Cow;
use tera::Context;
use warp::http::{header::CONTENT_TYPE, response::Response, status::StatusCode};

const CONTENT_TYPE_HTML: &str = "text/html; charset=UTF-8";

/// When making using a custom status, use a closure that coerces to a `fn(&Self) -> Status`
#[macro_export]
macro_rules! impl_webpage {
    ($page:ty = $template:literal $(, status = $status:expr)? $(, content_type = $content_type:expr)? $(,)?) => {
        impl_webpage!($page = |_| ::std::borrow::Cow::Borrowed($template) $(, status = $status)? $(, content_type = $content_type)?);
    };

    ($page:ty = $template:expr $(, status = $status:expr)? $(, content_type = $content_type:expr)? $(,)?) => {
        impl $crate::web::page::WebPage for $page {
            fn template(&self) -> ::std::borrow::Cow<'static, str> {
                let template: fn(&Self) -> ::std::borrow::Cow<'static, str> = $template;
                template(self)
            }

            $(
                fn get_status(&self) -> ::warp::http::status::StatusCode {
                    let status: fn(&Self) -> ::warp::http::status::StatusCode = $status;
                    (status)(self)
                }
            )?

            $(
                fn content_type() -> &'static str {
                    $content_type
                }
            )?
        }
    };
}

/// The central trait that rendering pages revolves around, it handles selecting and rendering the template
pub trait WebPage: Serialize + Sized {
    /// Turn the current instance into a `Response`, ready to be served
    // TODO: We could cache similar pages using the `&Context`
    fn into_response(self, template_data: &TemplateData) -> DocsrsResult<Response<String>> {
        let ctx = Context::from_serialize(&self).map_err(|error| DocsrsError::Template {
            template_name: self.template(),
            error,
        })?;
        let rendered = template_data
            .templates
            .load()
            .render(&self.template(), &ctx)
            .map_err(|error| DocsrsError::Template {
                template_name: self.template(),
                error,
            })?;

        let mut response = Response::builder()
            .status(self.get_status())
            .header(CONTENT_TYPE, Self::content_type())
            .body(rendered)
            .expect("invalid response header");

        Ok(response)
    }

    /// The name of the template to be rendered
    fn template(&self) -> Cow<'static, str>;

    /// Gets the status of the request, defaults to `Ok`
    fn get_status(&self) -> StatusCode {
        StatusCode::OK
    }

    /// The content type that the template should be served with, defaults to html
    fn content_type() -> &'static str {
        CONTENT_TYPE_HTML
    }
}
