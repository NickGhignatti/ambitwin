use claims_schema::CLAIMS_HEADER;
use std::time::Duration;

// a hung digital-twin raises no error, so without a timeout every save would hang with it
const TIMEOUT: Duration = Duration::from_secs(2);

/// digital-twin, asked which domains a building belongs to. Only it knows; dashboard keeps no copy
/// that could go stale when a building changes hands.
#[derive(Clone)]
pub struct TwinDirectory {
    base_url: String,
    client: reqwest::Client,
}

impl TwinDirectory {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::builder()
                .timeout(TIMEOUT)
                .build()
                .expect("reqwest client builds"),
        }
    }

    /// The building's domains, asked with the caller's own claims header; empty for an unknown
    /// building. Errors when digital-twin is unreachable, slow, or answers anything but `2xx`.
    pub async fn domains_of(
        &self,
        building_id: &str,
        claims_header: &str,
    ) -> anyhow::Result<Vec<String>> {
        let url = format!(
            "{}/domain/{}",
            self.base_url,
            urlencoding::encode(building_id)
        );
        let response = self
            .client
            .get(&url)
            .header(CLAIMS_HEADER, claims_header)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::TwinDirectory;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn asks_with_the_callers_claims_and_an_encoded_building_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/domain/main%20hall"))
            .and(header("x-gateway-claims", "caller-claims"))
            .respond_with(ResponseTemplate::new(200).set_body_json(["eng", "sales"]))
            .mount(&server)
            .await;

        let domains = TwinDirectory::new(server.uri())
            .domains_of("main hall", "caller-claims")
            .await
            .unwrap();
        assert_eq!(domains, ["eng", "sales"]);
    }

    #[tokio::test]
    async fn a_failed_answer_is_an_error_not_an_empty_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let answer = TwinDirectory::new(server.uri())
            .domains_of("b1", "caller-claims")
            .await;
        assert!(answer.is_err());
    }
}
