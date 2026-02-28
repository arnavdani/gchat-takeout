use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleCreator {
    pub name: String,
    pub email: Option<String>,
    pub user_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleAttachment {
    pub original_name: String,
    pub export_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleMessage {
    pub creator: GoogleCreator,
    pub created_date: String,
    pub text: Option<String>,
    pub topic_id: Option<String>,
    pub message_id: String,
    pub attached_files: Option<Vec<GoogleAttachment>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleMessagesFile {
    pub messages: Vec<GoogleMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleMember {
    pub name: String,
    pub email: Option<String>,
    pub user_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleGroupInfo {
    pub name: Option<String>,
    pub members: Vec<GoogleMember>,
}
