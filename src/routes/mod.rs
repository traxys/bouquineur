use maud::{html, Markup};

use crate::State;

#[derive(thiserror::Error, Debug)]
enum RouteError {
    #[error("Database error")]
    Db(#[from] diesel::result::Error),
}

#[derive(PartialEq, Eq)]
enum Page {
    Books,
    AddBook,
}

impl Page {
    fn variants() -> &'static [Self] {
        &[Self::Books, Self::AddBook]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Page::Books => "Books",
            Page::AddBook => "Add a Book",
        }
    }

    pub fn location(&self) -> &'static str {
        match self {
            Page::Books => "/",
            Page::AddBook => "/add",
        }
    }
}

fn base_page_with_head(body: Markup, head: Option<Markup>) -> Markup {
    html! {
        (maud::DOCTYPE)
        html lang="en" data-bs-theme="dark" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Bouquineur" }
                link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/css/bootstrap.min.css"
                     rel="stylesheet"
                     integrity="sha384-T3c6CoIi6uLrA9TneNEoa7RxnatzjcDSCmG1MXxSR1GAsXEV/Dwwykc2MPK8M2HN"
                     crossorigin="anonymous";
                link rel="stylesheet"
                     href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.11.3/font/bootstrap-icons.min.css"
                     integrity="sha384-XGjxtQfXaH2tnPFa9x+ruJTuLE3Aa6LhHSWRr1XeTyhezb4abCG4ccI5AkVDxqC+"
                     crossorigin="anonymous";
                @if let Some(head) = head {
                    (head)
                }
            }
            body {
                (body)
                script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/js/bootstrap.bundle.min.js"
                       integrity="sha384-C6RzsynM9kWDrMNeT87bh95OGNyZPhcTNXj1NW7RuBCsyN/o0jlpcV8Qyq46cDfL"
                       crossorigin="anonymous" {}
            }
        }
    }
}

fn base_page(body: Markup) -> Markup {
    base_page_with_head(body, None)
}

fn app_page(body: Markup, page: Page) -> Markup {
    base_page(html! {
        .container-fluid {
            header .d-flex.justify-content-center."py-3" {
                ul .nav.nav-pills {
                    @for p in Page::variants() {
                        @let current = *p == page;
                        li .nav-item {
                            a .nav-link.active[current]
                                aria-current=[current.then(|| "page")]
                                href=(p.location()) {
                                (p.name())
                            }
                        }
                    }
                }
            }
            (body)
        }
    })
}

pub async fn index(state: State) -> maud::Markup {
    app_page(
        html! {
            p { "Hello!" }
        },
        Page::Books,
    )
}
