use actix_web::{
    middleware::Logger,
    web, App, HttpResponse, HttpServer, Responder,
    post, get, put, delete,
    error::ResponseError,
    http::StatusCode,
};
use dotenv::dotenv;
use env_logger;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool, FromRow};
use std::fmt;

// -------------------- DB --------------------

pub async fn establish_connection() -> Result<PgPool, sqlx::Error> {
    let database_url = "postgres://postgres:password@localhost:5432/rust";
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

// -------------------- Models --------------------

#[derive(Serialize, Deserialize, Debug, FromRow)]
pub struct BlogPost {
    pub id: i32,
    pub title: String,
    pub author: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewBlogPost {
    pub title: String,
    pub author: String,
    pub content: String,
}

// -------------------- API Error --------------------

#[derive(Debug)]
pub enum ApiError {
    DatabaseError(String),
    NotFound(String),
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::DatabaseError(msg) => {
                HttpResponse::InternalServerError().json(msg)
            }
            ApiError::NotFound(msg) => {
                HttpResponse::NotFound().json(msg)
            }
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::DatabaseError(msg) => write!(f, "Database Error: {}", msg),
            ApiError::NotFound(msg) => write!(f, "Not Found: {}", msg),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => {
                ApiError::NotFound("Record not found".to_string())
            }
            _ => ApiError::DatabaseError(err.to_string()),
        }
    }
}

// -------------------- SQLX --------------------

pub async fn create_post(
    pool: &PgPool,
    post: &NewBlogPost,
) -> Result<BlogPost, ApiError> {
    sqlx::query_as::<_, BlogPost>(
        r#"
        INSERT INTO blog_posts (title, content, author)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
    )
    .bind(&post.title)
    .bind(&post.content)
    .bind(&post.author)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn get_all_posts(pool: &PgPool) -> Result<Vec<BlogPost>, ApiError> {
    sqlx::query_as::<_, BlogPost>("SELECT * FROM blog_posts")
        .fetch_all(pool)
        .await
        .map_err(ApiError::from)
}

pub async fn get_post(pool: &PgPool, id: i32) -> Result<BlogPost, ApiError> {
    sqlx::query_as::<_, BlogPost>(
        "SELECT * FROM blog_posts WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn update_post(
    pool: &PgPool,
    id: i32,
    post: &NewBlogPost,
) -> Result<impl Responder, ApiError> {
    sqlx::query(
        "UPDATE blog_posts SET title=$1, content=$2, author=$3 WHERE id=$4",
    )
    .bind(&post.title)
    .bind(&post.content)
    .bind(&post.author)
    .bind(id)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;

     Ok(HttpResponse::Ok().json(&post))
}

pub async fn delete_post(pool: &PgPool, id: i32) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM blog_posts WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(())
}

// -------------------- Routes --------------------

async fn index_page() -> &'static str {
    "Hello Crud API"
}

#[post("/blog")]
async fn create_blogpost(
    pool: web::Data<PgPool>,
    new_post: web::Json<NewBlogPost>,
) -> Result<impl Responder, ApiError> {
    let post = create_post(&pool, &new_post).await?;
    Ok(HttpResponse::Ok().json(post))
}

#[get("/blog")]
async fn get_blogposts(
    pool: web::Data<PgPool>,
) -> Result<impl Responder, ApiError> {
    let posts = get_all_posts(&pool).await?;
    Ok(HttpResponse::Ok().json(posts))
}

#[get("/blog/{id}")]
async fn get_blogpost(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> Result<impl Responder, ApiError> {
    let post = get_post(&pool, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(post))
}

#[put("/blog/{id}")]
async fn update_blogpost(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    updated_post: web::Json<NewBlogPost>,
) -> Result<impl Responder, ApiError> {
    update_post(&pool, path.into_inner(), &updated_post).await?;
    Ok(HttpResponse::Ok().finish())
}

#[delete("/blog/{id}")]
async fn delete_blogpost(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> Result<impl Responder, ApiError> {
    delete_post(&pool, path.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

// -------------------- Main --------------------

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();

    let pool = establish_connection()
        .await
        .expect("Failed to connect to database");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .wrap(Logger::default())
            .route("/", web::get().to(index_page))
            .service(create_blogpost)
            .service(get_blogposts)
            .service(get_blogpost)
            .service(update_blogpost)
            .service(delete_blogpost)
    })
    .bind(("127.0.0.1", 8081))?
    .run()
    .await
}
