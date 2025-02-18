use crate::{
    request_hash, response_hash, DefaultCelBuilder, DefaultFullCelExpression,
    DefaultResponseOnlyCelExpression, HttpCertificationResult, HttpRequest, HttpResponse,
};
use ic_certification::Hash;
use ic_representation_independent_hash::hash;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpCertificationType {
    Skip {
        cel_expr_hash: Hash,
    },
    ResponseOnly {
        cel_expr_hash: Hash,
        response_hash: Hash,
    },
    Full {
        cel_expr_hash: Hash,
        request_hash: Hash,
        response_hash: Hash,
    },
}

/// A certified [request](crate::HttpResponse) and [response](crate::HttpResponse) pair.
///
/// It supports three types of certification via associated functions:
///
/// - [skip()](HttpCertification::skip()) excludes both an [HTTP request](crate::HttpRequest) and the
/// corresponding [HTTP response](crate::HttpResponse) from certification.
///
/// - [response_only()](HttpCertification::response_only()) includes an
/// [HTTP response](crate::HttpResponse) but excludes the corresponding [HTTP request](crate::HttpRequest)
/// from certification.
///
/// - [full()](HttpCertification::full()) includes both an [HTTP response](crate::HttpResponse) and
/// the corresponding [HTTP request](crate::HttpRequest) in certification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpCertification(HttpCertificationType);

impl HttpCertification {
    /// Creates a certification that excludes both the [HTTP request](crate::HttpRequest) and
    /// the corresponding [HTTP response](crate::HttpResponse).
    pub fn skip() -> HttpCertification {
        let cel_expr = DefaultCelBuilder::skip_certification().to_string();
        let cel_expr_hash = hash(cel_expr.as_bytes());

        Self(HttpCertificationType::Skip { cel_expr_hash })
    }

    /// Creates a certification that includes an [HTTP response](crate::HttpResponse), but excludes the
    /// corresponding [HTTP request](crate::HttpRequest).
    pub fn response_only(
        cel_expr: &DefaultResponseOnlyCelExpression,
        response: &HttpResponse,
        response_body_hash: Option<Hash>,
    ) -> HttpCertification {
        let cel_expr_hash = hash(cel_expr.to_string().as_bytes());
        let response_hash = response_hash(response, &cel_expr.response, response_body_hash);

        Self(HttpCertificationType::ResponseOnly {
            cel_expr_hash,
            response_hash,
        })
    }

    /// Creates a certification that includes both an [HTTP response](crate::HttpResponse) and the corresponding
    /// [HTTP request](crate::HttpRequest).
    pub fn full(
        cel_expr: &DefaultFullCelExpression,
        request: &HttpRequest,
        response: &HttpResponse,
        response_body_hash: Option<Hash>,
    ) -> HttpCertificationResult<HttpCertification> {
        let cel_expr_hash = hash(cel_expr.to_string().as_bytes());
        let request_hash = request_hash(request, &cel_expr.request)?;
        let response_hash = response_hash(response, &cel_expr.response, response_body_hash);

        Ok(Self(HttpCertificationType::Full {
            cel_expr_hash,
            request_hash,
            response_hash,
        }))
    }

    pub(crate) fn to_tree_path(self) -> Vec<Vec<u8>> {
        match self.0 {
            HttpCertificationType::Skip { cel_expr_hash } => vec![cel_expr_hash.to_vec()],
            HttpCertificationType::ResponseOnly {
                cel_expr_hash,
                response_hash,
            } => vec![
                cel_expr_hash.to_vec(),
                "".as_bytes().to_vec(),
                response_hash.to_vec(),
            ],
            HttpCertificationType::Full {
                cel_expr_hash,
                request_hash,
                response_hash,
            } => vec![
                cel_expr_hash.to_vec(),
                request_hash.to_vec(),
                response_hash.to_vec(),
            ],
        }
    }
}

impl<'a> From<HttpCertification> for Cow<'a, HttpCertification> {
    fn from(cert: HttpCertification) -> Cow<'a, HttpCertification> {
        Cow::Owned(cert)
    }
}

impl<'a> From<&'a HttpCertification> for Cow<'a, HttpCertification> {
    fn from(cert: &'a HttpCertification) -> Cow<'a, HttpCertification> {
        Cow::Borrowed(cert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DefaultResponseCertification;
    use rstest::*;

    #[rstest]
    fn no_certification() {
        let cel_expr = DefaultCelBuilder::skip_certification().to_string();
        let expected_cel_expr_hash = hash(cel_expr.as_bytes());

        let result = HttpCertification::skip();

        assert!(matches!(
            result.0,
            HttpCertificationType::Skip { cel_expr_hash } if cel_expr_hash == expected_cel_expr_hash
        ));
        assert_eq!(result.to_tree_path(), vec![expected_cel_expr_hash.to_vec()]);
    }

    #[rstest]
    fn response_only_certification() {
        let cel_expr = DefaultCelBuilder::response_only_certification()
            .with_response_certification(DefaultResponseCertification::certified_response_headers(
                vec!["ETag", "Cache-Control"],
            ))
            .build();
        let expected_cel_expr_hash = hash(cel_expr.to_string().as_bytes());

        let response = &HttpResponse {
            status_code: 200,
            body: vec![],
            headers: vec![],
            upgrade: None,
        };
        let expected_response_hash = response_hash(response, &cel_expr.response, None);

        let result = HttpCertification::response_only(&cel_expr, response, None);

        assert!(matches!(
            result.0,
            HttpCertificationType::ResponseOnly {
                cel_expr_hash,
                response_hash
            } if cel_expr_hash == expected_cel_expr_hash &&
                response_hash == expected_response_hash
        ));
        assert_eq!(
            result.to_tree_path(),
            vec![
                expected_cel_expr_hash.to_vec(),
                "".as_bytes().to_vec(),
                expected_response_hash.to_vec()
            ]
        );
    }

    #[rstest]
    fn full_certification() {
        let cel_expr = DefaultCelBuilder::full_certification()
            .with_request_headers(vec!["If-Match"])
            .with_request_query_parameters(vec!["foo", "bar", "baz"])
            .with_response_certification(DefaultResponseCertification::certified_response_headers(
                vec!["ETag", "Cache-Control"],
            ))
            .build();
        let expected_cel_expr_hash = hash(cel_expr.to_string().as_bytes());

        let request = &HttpRequest {
            body: vec![],
            headers: vec![],
            method: "GET".to_string(),
            url: "/index.html".to_string(),
        };
        let expected_request_hash = request_hash(request, &cel_expr.request).unwrap();

        let response = &HttpResponse {
            status_code: 200,
            body: vec![],
            headers: vec![],
            upgrade: None,
        };
        let expected_response_hash = response_hash(response, &cel_expr.response, None);

        let result = HttpCertification::full(&cel_expr, request, response, None).unwrap();

        assert!(matches!(
            result.0,
            HttpCertificationType::Full {
                cel_expr_hash,
                request_hash,
                response_hash
            } if cel_expr_hash == expected_cel_expr_hash &&
                request_hash == expected_request_hash &&
                response_hash == expected_response_hash
        ));
        assert_eq!(
            result.to_tree_path(),
            vec![
                expected_cel_expr_hash.to_vec(),
                expected_request_hash.to_vec(),
                expected_response_hash.to_vec()
            ]
        );
    }
}
