use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CpaTokenRecord {
    pub account_id: String,
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub id_token: String,
    #[serde(default)]
    pub email: String,
    #[serde(default, rename = "expired")]
    pub expired_at: String,
    #[serde(default, rename = "type")]
    pub account_type: String,
}

impl CpaTokenRecord {
    pub fn has_required_fields(&self) -> bool {
        !self.account_id.trim().is_empty() && !self.access_token.trim().is_empty()
    }

    pub fn build_name(&self, index: usize) -> String {
        if !self.email.trim().is_empty() {
            return self.email.trim().to_string();
        }

        let account_type = if self.account_type.trim().is_empty() {
            "unknown"
        } else {
            self.account_type.trim()
        };

        format!("{account_type}-普号-{index:04}")
    }
}
