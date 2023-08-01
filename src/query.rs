use crate::read::reverse::LineResult;
use axum::http::StatusCode;
use axum::http::Uri;
use querystring::querify;
use std::collections::HashMap;
use std::str::FromStr;

const SUBSTRINGS_PARAMETER: &str = "substring[]";
const LIMIT_PARAMETER: &str = "limit";

/// Container for the various query parameters that may be applied to an /inspect request.
#[derive(Debug, PartialEq, Eq, Default)]
pub struct InspectQuery {
    substrings: Vec<String>,
    limit: Option<usize>,
}

impl InspectQuery {
    pub fn new(substrings: Vec<String>, limit: usize) -> Self {
        Self {
            substrings,
            limit: Some(limit),
        }
    }

    pub fn with_substrings(substrings: Vec<String>) -> Self {
        Self {
            substrings,
            limit: None,
        }
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            substrings: Vec::default(),
            limit: Some(limit),
        }
    }
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

        let limit = match query.get(LIMIT_PARAMETER) {
            Some(values) => match values.iter().last() {
                Some(value) => match usize::from_str(value) {
                    Ok(l) => Some(l),
                    Err(_) => {
                        return Err(StatusCode::BAD_REQUEST);
                    }
                },
                None => None,
            },
            None => None,
        };

        Ok(InspectQuery {
            substrings: query
                .get(SUBSTRINGS_PARAMETER)
                .cloned()
                .unwrap_or_else(|| Vec::default()),
            limit,
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

    pub(crate) fn limit(&self) -> Option<usize> {
        self.limit
    }

    pub fn to_uri(&self) -> String {
        let mut query = if self.substrings.is_empty() {
            "".to_string()
        } else {
            let mut query = "?".to_string();

            for (i, substring) in self.substrings.iter().enumerate() {
                query.push_str(SUBSTRINGS_PARAMETER);
                query.push_str("=");
                query.push_str(urlencoding::encode(substring).as_ref());

                if i + 1 < self.substrings.len() {
                    query.push_str("&");
                }
            }

            query
        };

        if let Some(l) = self.limit {
            if query.is_empty() {
                query.push_str("?");
            } else {
                query.push_str("&");
            }

            query.push_str(LIMIT_PARAMETER);
            query.push_str("=");
            query.push_str(l.to_string().as_str());
        }

        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::str::FromStr;

    #[rstest]
    #[case("/some/path", true)]
    #[case("/some/path?moot", false)]
    #[case("/some/path?moot=moot", false)]
    #[case("/some/path?moot&something=else", false)]
    #[case("/some/path?moot=moot&something=else", false)]
    #[test]
    fn inspect_query_empty(#[case] uri: &str, #[case] inverts: bool) {
        // Setup
        let uri = Uri::from_str(uri).unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap();

        // Verify
        assert_eq!(
            result,
            InspectQuery {
                substrings: Vec::default(),
                limit: None,
            }
        );

        if inverts {
            assert_eq!(invert("/some/path", result), uri);
        }
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
                limit: None,
            }
        );
        assert_eq!(invert("/some/path", result), uri);
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
                limit: None,
            }
        );
        assert_eq!(invert("/some/path", result), uri);
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
                limit: None,
            }
        );
        assert_eq!(invert("/some/path", result), uri);
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
                limit: None,
            }
        );
        assert_eq!(invert("/some/path", result), uri);
    }

    #[test]
    fn inspect_query_limit() {
        // Setup
        let uri = Uri::from_str(format!("/some/path?{LIMIT_PARAMETER}=123").as_str()).unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap();

        // Verify
        assert_eq!(
            result,
            InspectQuery {
                substrings: Vec::default(),
                limit: Some(123),
            }
        );
        assert_eq!(invert("/some/path", result), uri);
    }

    #[test]
    fn inspect_query_limit_invalid() {
        // Setup
        let uri = Uri::from_str(format!("/some/path?{LIMIT_PARAMETER}=abc").as_str()).unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap_err();

        // Verify
        assert_eq!(result, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn inspect_query_limit_multiple() {
        // Setup
        let uri = Uri::from_str(
            format!("/some/path?{LIMIT_PARAMETER}=123&{LIMIT_PARAMETER}=456").as_str(),
        )
        .unwrap();

        // Execute
        let result = InspectQuery::try_from(&uri).unwrap();

        // Verify
        assert_eq!(
            result,
            InspectQuery {
                substrings: Vec::default(),
                limit: Some(456),
            }
        );
    }

    #[test]
    fn line_matches_empty() {
        let query = InspectQuery {
            substrings: Vec::default(),
            limit: None,
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
            limit: None,
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
            limit: None,
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
            limit: None,
        };

        assert!(query.line_matches(&LineResult(Ok("1".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("2".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("3".to_string()))));
        assert!(query.line_matches(&LineResult(Ok("123".to_string()))));

        assert!(!query.line_matches(&LineResult(Ok("4".to_string()))));

        // The filter should always pass for errors - we want these to propagate further down the stream.
        assert!(query.line_matches(&LineResult(Err(()))));
    }

    #[test]
    fn limit() {
        let query = InspectQuery {
            substrings: Vec::default(),
            limit: None,
        };
        assert_eq!(query.limit(), None);

        let query = InspectQuery {
            substrings: Vec::default(),
            limit: Some(123),
        };
        assert_eq!(query.limit(), Some(123));
    }

    fn invert(path: &str, query: InspectQuery) -> Uri {
        Uri::from_str(format!("{path}{}", query.to_uri()).as_str()).unwrap()
    }
}
