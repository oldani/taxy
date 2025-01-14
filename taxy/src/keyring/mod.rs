use taxy_api::cert::KeyringInfo;

use self::{acme::AcmeEntry, certs::Cert};
use std::{collections::HashMap, sync::Arc};

pub mod acme;
pub mod certs;

#[derive(Debug, Default)]
pub struct Keyring {
    certs: HashMap<String, KeyringItem>,
}

#[derive(Debug)]
pub enum KeyringItem {
    ServerCert(Arc<Cert>),
    Acme(Arc<AcmeEntry>),
}

impl KeyringItem {
    pub fn id(&self) -> &str {
        match self {
            Self::ServerCert(cert) => cert.id(),
            Self::Acme(acme) => acme.id(),
        }
    }

    pub fn info(&self) -> KeyringInfo {
        match self {
            Self::ServerCert(cert) => KeyringInfo::ServerCert(cert.info()),
            Self::Acme(acme) => KeyringInfo::Acme(acme.info()),
        }
    }
}

impl Keyring {
    pub fn new<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = KeyringItem>,
    {
        Self {
            certs: iter
                .into_iter()
                .map(|cert| (cert.id().to_string(), cert))
                .collect(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &KeyringItem> {
        self.certs.values()
    }

    pub fn certs(&self) -> Vec<Arc<Cert>> {
        let mut certs = self
            .certs
            .values()
            .filter_map(|item| match item {
                KeyringItem::ServerCert(cert) => Some(cert.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        certs.sort();
        certs
    }

    pub fn acme_entries(&self) -> Vec<&Arc<AcmeEntry>> {
        self.certs
            .values()
            .filter_map(|item| match item {
                KeyringItem::Acme(acme) => Some(acme),
                _ => None,
            })
            .collect::<Vec<_>>()
    }

    pub fn find_server_certs_by_acme(&self, acme: &str) -> Vec<&Arc<Cert>> {
        let mut certs = self
            .certs
            .values()
            .filter_map(|item| match item {
                KeyringItem::ServerCert(cert) => Some(cert),
                _ => None,
            })
            .filter(|cert| {
                cert.metadata
                    .as_ref()
                    .map_or(false, |meta| meta.acme_id == acme)
            })
            .collect::<Vec<_>>();
        certs.sort();
        certs
    }

    pub fn add(&mut self, item: KeyringItem) {
        self.certs.insert(item.id().to_string(), item);
    }

    pub fn delete(&mut self, id: &str) -> Option<KeyringItem> {
        self.certs.remove(id)
    }

    pub fn list(&self) -> Vec<KeyringInfo> {
        let mut list = self
            .certs
            .values()
            .map(|cert| cert.info())
            .collect::<Vec<_>>();
        list.sort_unstable_by_key(|cert| cert.id().to_string());
        list
    }
}
