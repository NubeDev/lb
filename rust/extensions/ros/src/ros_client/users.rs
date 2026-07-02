//! The `user` model + the `/api/users` list. Vendored verbatim (`async` port); not on the driver's
//! critical path but kept so the client is a faithful copy of the box's REST surface.

use serde::{Deserialize, Serialize};

use super::{client::Client, error::RosClientError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub uuid: String,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsersResponse {
    data: Vec<User>,
}

impl Client {
    pub async fn get_users(&self) -> Result<Vec<User>, RosClientError> {
        let response: UsersResponse = self.get_json("/api/users", &[]).await?;
        Ok(response.data)
    }
}
