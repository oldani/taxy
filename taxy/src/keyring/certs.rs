use pkcs8::{PrivateKeyInfo, SecretDocument};
use rcgen::{BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, SanType};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use taxy_api::cert::{CertInfo, CertMetadata, SelfSignedCertRequest};
use taxy_api::error::Error;
use taxy_api::subject_name::SubjectName;
use tokio_rustls::rustls::sign::CertifiedKey;
use tokio_rustls::rustls::{sign, Certificate, PrivateKey};
use tracing::error;
use x509_parser::{extensions::GeneralName, time::ASN1Time};
use x509_parser::{parse_x509_certificate, prelude::X509Certificate};

const CERT_ID_LENGTH: usize = 20;

#[derive(Clone)]
pub struct Cert {
    pub id: String,
    pub key: SecretDocument,
    pub raw_chain: Vec<u8>,
    pub raw_key: Vec<u8>,
    pub fingerprint: String,
    pub issuer: String,
    pub root_cert: Option<String>,
    pub san: Vec<SubjectName>,
    pub not_after: ASN1Time,
    pub not_before: ASN1Time,
    pub metadata: Option<CertMetadata>,
}

impl PartialEq for Cert {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }
}

impl Eq for Cert {}

impl fmt::Debug for Cert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cert")
            .field("id", &self.id)
            .field("fingerprint", &self.fingerprint)
            .field("issuer", &self.issuer)
            .field("root_cert", &self.root_cert)
            .field("san", &self.san)
            .field("not_after", &self.not_after)
            .field("not_before", &self.not_before)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl PartialOrd for Cert {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(
            other
                .metadata
                .as_ref()
                .map(|meta| meta.is_trusted)
                .unwrap_or_default()
                .partial_cmp(
                    &self
                        .metadata
                        .as_ref()
                        .map(|meta| meta.is_trusted)
                        .unwrap_or_default(),
                )
                .unwrap()
                .then_with(|| other.not_before.partial_cmp(&self.not_before).unwrap())
                .then_with(|| self.not_after.partial_cmp(&other.not_after).unwrap())
                .then_with(|| self.fingerprint.cmp(&other.fingerprint)),
        )
    }
}

impl Ord for Cert {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Cert {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn info(&self) -> CertInfo {
        CertInfo {
            id: self.id.clone(),
            fingerprint: self.fingerprint.clone(),
            issuer: self.issuer.clone(),
            root_cert: self.root_cert.clone(),
            san: self.san.clone(),
            not_after: self.not_after.timestamp(),
            not_before: self.not_before.timestamp(),
            metadata: self.metadata.clone(),
        }
    }

    pub fn is_valid(&self) -> bool {
        let now = ASN1Time::now();
        self.not_before <= now && now <= self.not_after
    }

    pub fn has_subject_name(&self, name: &SubjectName) -> bool {
        for san in &self.san {
            if match (san, name) {
                (SubjectName::DnsName(c), SubjectName::DnsName(n)) => c == n,
                (SubjectName::WildcardDnsName(c), SubjectName::DnsName(n)) => {
                    c == n.trim_start_matches(|c| c != '.').trim_start_matches('.')
                }
                (SubjectName::WildcardDnsName(c), SubjectName::WildcardDnsName(n)) => c == n,
                (SubjectName::IPAddress(c), SubjectName::IPAddress(n)) => c == n,
                _ => false,
            } {
                return true;
            }
        }
        false
    }

    pub fn new(raw_chain: Vec<u8>, raw_key: Vec<u8>) -> Result<Self, Error> {
        let key_pem =
            std::str::from_utf8(&raw_key).map_err(|_| Error::FailedToDecryptPrivateKey)?;
        let (_, key) =
            SecretDocument::from_pem(key_pem).map_err(|_| Error::FailedToDecryptPrivateKey)?;

        let chain_meta = raw_chain.as_slice();
        let mut meta_read = BufReader::new(chain_meta);
        let mut comment = String::new();
        meta_read
            .read_line(&mut comment)
            .map_err(|_| Error::FailedToReadCertificate)?;

        let metadata: Option<CertMetadata> = serde_qs::from_str(
            comment
                .trim_start_matches(|c: char| c == '#' || c.is_whitespace())
                .trim_end(),
        )
        .ok();

        let mut chain = raw_chain.as_slice();
        let chain =
            rustls_pemfile::certs(&mut chain).map_err(|_| Error::FailedToReadCertificate)?;
        let chain = chain.into_iter().map(Certificate).collect::<Vec<_>>();

        let der = &chain.first().ok_or(Error::FailedToReadCertificate)?.0;
        let mut hasher = Sha256::new();
        hasher.update(der);
        let fingerprint = hex::encode(hasher.finalize());

        let parsed_chain = parse_chain(&chain)?;
        let x509 = parsed_chain.first().ok_or(Error::FailedToReadCertificate)?;
        let san = x509
            .subject_alternative_name()
            .into_iter()
            .flatten()
            .flat_map(|name| &name.value.general_names)
            .filter_map(|name| match name {
                GeneralName::DNSName(name) => SubjectName::from_str(name).ok(),
                _ => None,
            })
            .collect();

        let not_after = x509.validity().not_after;
        let not_before = x509.validity().not_before;

        let issuer = x509.issuer().to_string();
        let root_cert = parsed_chain
            .last()
            .filter(|_| chain.len() > 1)
            .map(|cert| cert.subject().to_string());

        Ok(Self {
            id: fingerprint[..CERT_ID_LENGTH].to_string(),
            fingerprint,
            key,
            raw_chain,
            raw_key,
            issuer,
            root_cert,
            san,
            not_after,
            not_before,
            metadata,
        })
    }

    pub fn new_self_signed(req: &SelfSignedCertRequest) -> Result<Self, Error> {
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, "Taxy CA");
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.distinguished_name = distinguished_name;

        let ca_cert = match rcgen::Certificate::from_params(ca_params) {
            Ok(cert) => cert,
            Err(err) => {
                error!(?err);
                return Err(Error::FailedToGerateSelfSignedCertificate);
            }
        };

        let mut params = CertificateParams::default();
        params.subject_alt_names = req
            .san
            .iter()
            .map(|name| {
                if let SubjectName::IPAddress(ip) = name {
                    SanType::IpAddress(*ip)
                } else {
                    SanType::DnsName(name.to_string())
                }
            })
            .collect();

        let common_name = req
            .san
            .iter()
            .map(|name| name.to_string())
            .next()
            .unwrap_or_else(|| "Taxy Cert".into());
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, common_name);
        params.distinguished_name = distinguished_name;

        let cert = match rcgen::Certificate::from_params(params) {
            Ok(cert) => cert,
            Err(err) => {
                error!(?err);
                return Err(Error::FailedToGerateSelfSignedCertificate);
            }
        };

        let cert_pem = cert
            .serialize_pem_with_signer(&ca_cert)
            .map_err(|_| Error::FailedToGerateSelfSignedCertificate)?;

        let ca_pem = ca_cert
            .serialize_pem()
            .map_err(|_| Error::FailedToGerateSelfSignedCertificate)?;

        let raw_chain = format!("{}\r\n{}", cert_pem, ca_pem).into_bytes();
        let raw_key = cert.serialize_private_key_pem().into_bytes();

        Self::new(raw_chain, raw_key)
    }

    pub fn certified(&self) -> Result<CertifiedKey, Error> {
        match self.certified_impl() {
            Ok(certified) => Ok(certified),
            Err(err) => {
                error!(?err);
                Err(Error::FailedToDecryptPrivateKey)
            }
        }
    }

    fn certified_impl(&self) -> anyhow::Result<CertifiedKey> {
        let key = self
            .key
            .decode_msg::<PrivateKeyInfo>()
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        let signing_key = sign::any_supported_type(&PrivateKey(key.private_key.to_vec()))
            .map_err(|err| anyhow::anyhow!("{err}"))?;

        let mut chain = self.raw_chain.as_slice();
        let chain =
            rustls_pemfile::certs(&mut chain).map_err(|_| Error::FailedToReadCertificate)?;
        let chain = chain.into_iter().map(Certificate).collect::<Vec<_>>();
        Ok(CertifiedKey::new(chain, signing_key))
    }
}

fn parse_chain(chain: &[Certificate]) -> Result<Vec<X509Certificate>, Error> {
    let mut certs = Vec::new();
    for data in chain {
        let (_, cert) =
            parse_x509_certificate(&data.0).map_err(|_| Error::FailedToReadCertificate)?;
        certs.push(cert);
    }
    Ok(certs)
}

#[cfg(test)]
mod test {
    

    #[test]
    fn test_self_signed() {
        use super::*;

        let req = SelfSignedCertRequest {
            san: vec![SubjectName::from_str("localhost").unwrap()],
        };
        let cert = Cert::new_self_signed(&req).unwrap();
        assert_eq!(cert.san, req.san);
    }
}
