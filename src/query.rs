use crate::read::reverse::LineResult;
use axum::http::StatusCode;
use axum::http::Uri;
use querystring::querify;
use std::collections::HashMap;

const SUBSTRINGS_PARAMETER: &str = "substring[]";

/// Container for the various query parameters that may be applied to an /inspect request.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InspectQuery {
    substrings: Vec<String>,
}

// Converts the query string from a `Uri` into an `InspectQuery`.
impl TryFrom<&Uri> for InspectQuery {
    type Error = StatusCode;

    fn try_from(uri: &Uri) -> Result<InspectQuery, Self::Error> {
        let query_params = uri.query().map(|qs| querify(qs));
        let mut query: HashMap<String, Vec<String>> = HashMap::default();

        if let Some(query_params) = query_params {
            for (k, v) in query_params.into_iter() {
                let values = query.entry(k.to_string()).or_insert(Vec::default());
                match urlencoding::decode(v).map(|s| s.to_string()) {
                    Ok(value) => {
                        values.push(value);
                    }
                    Err(_) => {
                        return Err(StatusCode::BAD_REQUEST);
                    }
                }
            }
        }

        Ok(InspectQuery {
            substrings: query
                .get(SUBSTRINGS_PARAMETER)
                .cloned()
                .unwrap_or_else(|| Vec::default()),
        })
    }
}

impl InspectQuery {
    pub(crate) fn line_matches(&self, line_result: &LineResult) -> bool {
        match line_result {
            LineResult(Err(_)) => true,
            LineResult(Ok(line)) => {
                if !self.substrings.is_empty() {
                    self.substrings.iter().any(|substr| line.contains(substr))
                } else {
                    true
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::str::FromStr;

    #[rstest]
    #[case("/some/path")]
    #[case("/some/path?moot")]
    #[case("/some/path?moot=moot")]
    #[case("/some/path?moot&something=else")]
    #[case("/some/path?moot=moot&something=else")]
    #[test]
    fn inspect_query_empty(#[case] uri: &str) {
        // Setup
        let uri = Uri::from_str(uri).unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap();

        // Verify
        assert_eq!(
            result,
            InspectQuery {
                substrings: Vec::default(),
            }
        );
    }

    #[test]
    fn inspect_query_substring() {
        // Setup
        let uri = Uri::from_str(format!("/some/path?{SUBSTRINGS_PARAMETER}=123").as_str()).unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap();

        // Verify
        assert_eq!(
            result,
            InspectQuery {
                substrings: vec!["123".to_string()],
            }
        );
    }

    #[test]
    fn inspect_query_substring_multiple() {
        // Setup
        let uri = Uri::from_str(
            format!("/some/path?{SUBSTRINGS_PARAMETER}=123&{SUBSTRINGS_PARAMETER}=abc").as_str(),
        )
        .unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap();

        // Verify
        assert_eq!(
            result,
            InspectQuery {
                substrings: vec!["123".to_string(), "abc".to_string()],
            }
        );
    }

    #[test]
    fn inspect_query_substring_spaces() {
        // Setup
        let uri =
            Uri::from_str(format!("/some/path?{SUBSTRINGS_PARAMETER}=123%20abc").as_str()).unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap();

        // Verify
        assert_eq!(
            result,
            InspectQuery {
                substrings: vec!["123 abc".to_string()],
            }
        );
    }

    #[test]
    fn inspect_query_substring_unicode() {
        // Setup
        let uri = Uri::from_str(format!("/some/path?{SUBSTRINGS_PARAMETER}=%F0%9F%91%8D").as_str())
            .unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap();

        // Verify
        assert_eq!(
            result,
            InspectQuery {
                substrings: vec!["👍".to_string()],
            }
        );
    }

    #[test]
    fn line_matches_empty() {
        let query = InspectQuery {
            substrings: Vec::default(),
        };

        // An empty substrings[] always passes.
        assert!(query.line_matches(&LineResult(Ok("moot".to_string()))));
        // The filter should always pass for errors - we want these to propagate further down the stream.
        assert!(query.line_matches(&LineResult(Err(()))));
    }

    #[test]
    fn line_matches() {
        let query = InspectQuery {
            substrings: vec!["moot".to_string()],
        };

        assert!(query.line_matches(&LineResult(Ok("moot".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("...moot".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("moot...".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("...moot...".to_string()))));

        assert!(!query.line_matches(&LineResult(Ok("other".to_string()))));

        // The filter should always pass for errors - we want these to propagate further down the stream.
        assert!(query.line_matches(&LineResult(Err(()))));
    }

    #[test]
    fn line_matches_unicode() {
        let query = InspectQuery {
            substrings: vec!["👍".to_string()],
        };

        assert!(query.line_matches(&LineResult(Ok("👍".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("...👍".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("👍...".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("...👍...".to_string()))));

        assert!(!query.line_matches(&LineResult(Ok("👎".to_string()))));

        // The filter should always pass for errors - we want these to propagate further down the stream.
        assert!(query.line_matches(&LineResult(Err(()))));
    }

    #[test]
    fn line_matches_any() {
        let query = InspectQuery {
            substrings: vec!["1".to_string(), "2".to_string(), "3".to_string()],
        };

        assert!(query.line_matches(&LineResult(Ok("1".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("2".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("3".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("123".to_string()))));

        assert!(!query.line_matches(&LineResult(Ok("4".to_string()))));

        // The filter should always pass for errors - we want these to propagate further down the stream.
        assert!(query.line_matches(&LineResult(Err(()))));
    }
}
