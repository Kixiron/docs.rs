use crate::{
    db::Pool,
    web::{page::WebPage, TemplateData},
    BuildQueue, Metrics,
};
use std::{borrow::Cow, collections::HashSet};
use warp::{http::Uri, Filter, Rejection, Reply};

pub(super) const DOC_RUST_LANG_ORG_REDIRECTS: &[&str] =
    &["alloc", "core", "proc_macro", "std", "test"];

fn maybe<T>(
) -> impl Filter<Extract = (Option<T>,), Error = Rejection> + Clone + Send + Sync + 'static
where
    T: std::str::FromStr + Send + 'static,
{
    warp::path::param::<T>()
        .map(Some)
        .or_else(|_| futures_util::future::ok((None,)))
}

pub(super) fn build_warp(
    connection_pool: Pool,
    metrics: Metrics,
    build_queue: BuildQueue,
    templates: TemplateData,
) {
    let mut blacklisted_prefixes = HashSet::new();
    let render = |template| template.into_response(&templates.clone());
    let pool = warp::any().map(|| connection_pool.clone());
    let metrics = warp::any().map(|| metrics.clone());
    let build_queue = warp::any().map(|| build_queue.clone());

    let home_page = warp::path::end()
        .and(pool)
        .map(super::releases::home_page)
        .map(render);

    let static_resources = {
        // Well known resources, robots.txt and favicon.ico support redirection, the sitemap.xml
        // must live at the site root:
        //   https://developers.google.com/search/reference/robots_txt#handling-http-result-codes
        //   https://support.google.com/webmasters/answer/183668?hl=en
        let robots_redirect = warp::path!("robots.txt")
            .map(|| warp::redirect(Uri::from_static("/-/static/robots.txt")));
        let favicon_redirect = warp::path!("favicon.ico")
            .map(|| warp::redirect(Uri::from_static("/-/static/favicon.ico")));
        let sitemap_xml = warp::path!("sitemap.xml")
            .and(pool)
            .map(super::sitemap::sitemap_handler)
            .map(render);

        // This should not need to be served from the root as we reference the inner path in links,
        // but clients might have cached the url and need to update it.
        let opensearch_redirect = warp::path!("opensearch.xml")
            .map(|| warp::redirect(Uri::from_static("/-/static/opensearch.xml")));

        // /-/static/:file
        let static_files = warp::path!("-" / "static" / String)
            .map(super::statics::static_handler)
            .map(render);

        robots_redirect
            .or(favicon_redirect)
            .or(sitemap_xml)
            .or(static_files)
    };

    let about = {
        blacklisted_prefixes.insert("about");

        // /about
        let index = warp::path::end().map(|| {
            super::sitemap::about_handler(templates.clone(), Cow::Borrowed("index")).map(render)
        });

        // /about/metrics
        let metrics = warp::path!("metrics")
            .and(metrics)
            .and(pool)
            .and(build_queue)
            .map(super::metrics::metrics_handler);

        // /about/builds
        let builds = warp::path!("builds")
            .and(pool)
            .map(super::sitemap::about_builds_handler)
            .map(render);

        // /about/:page
        let pages = warp::path!(String)
            .map(|tab| super::sitemap::about_handler(templates.clone(), Cow::Owned(tab)))
            .map(render);

        warp::path("about").and(index.or(metrics).or(builds).or(pages))
    };
}
