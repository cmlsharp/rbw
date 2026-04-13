use std::io::{BufRead as _, Write as _};

use anyhow::Context as _;

// ---------------------------------------------------------------------------
// CryptoProvider trait — abstraction for encrypt/decrypt (mockable for tests)
// ---------------------------------------------------------------------------

pub trait CryptoProvider {
    fn decrypt(
        &self,
        cipherstring: &str,
        entry_key: Option<&str>,
        org_id: Option<&str>,
    ) -> anyhow::Result<String>;

    fn encrypt(
        &self,
        plaintext: &str,
        org_id: Option<&str>,
    ) -> anyhow::Result<String>;
}

// ---------------------------------------------------------------------------
// AgentControl trait — agent lifecycle commands (sync, unlock, lock, etc.)
// ---------------------------------------------------------------------------

pub trait AgentControl {
    fn simple_action(
        &self,
        action: crate::protocol::Action,
    ) -> anyhow::Result<()>;

    fn version(&self) -> anyhow::Result<u32>;
}

// ---------------------------------------------------------------------------
// Sock — Unix socket connection to the rbw-agent (private)
// ---------------------------------------------------------------------------

struct Sock(std::os::unix::net::UnixStream);

impl Sock {
    fn connect() -> std::io::Result<Self> {
        Ok(Self(std::os::unix::net::UnixStream::connect(
            crate::dirs::socket_file(),
        )?))
    }

    fn send(
        &mut self,
        msg: &crate::protocol::Request,
    ) -> anyhow::Result<()> {
        let Self(sock) = self;
        sock.write_all(
            serde_json::to_string(msg)
                .context("failed to serialize message to agent")?
                .as_bytes(),
        )
        .context("failed to send message to agent")?;
        sock.write_all(b"\n")
            .context("failed to send message to agent")?;
        Ok(())
    }

    fn recv(&mut self) -> anyhow::Result<crate::protocol::Response> {
        let Self(sock) = self;
        let mut buf = std::io::BufReader::new(sock);
        let mut line = String::new();
        buf.read_line(&mut line)
            .context("failed to read message from agent")?;
        serde_json::from_str(&line)
            .context("failed to parse message from agent")
    }
}

// ---------------------------------------------------------------------------
// AgentClient — real implementation using the rbw-agent socket
// ---------------------------------------------------------------------------

pub struct AgentClient {
    environment: crate::protocol::Environment,
}

impl AgentClient {
    pub fn new(environment: crate::protocol::Environment) -> Self {
        Self { environment }
    }

    fn connect(&self) -> anyhow::Result<Sock> {
        Sock::connect().with_context(|| {
            let log = crate::dirs::agent_stderr_file();
            format!(
                "failed to connect to rbw-agent \
                (this often means that the agent failed to start; \
                check {} for agent logs)",
                log.display()
            )
        })
    }
}

impl CryptoProvider for AgentClient {
    fn decrypt(
        &self,
        cipherstring: &str,
        entry_key: Option<&str>,
        org_id: Option<&str>,
    ) -> anyhow::Result<String> {
        let mut sock = self.connect()?;
        sock.send(&crate::protocol::Request::new(
            self.environment.clone(),
            crate::protocol::Action::Decrypt {
                cipherstring: cipherstring.to_string(),
                entry_key: entry_key
                    .map(std::string::ToString::to_string),
                org_id: org_id.map(std::string::ToString::to_string),
            },
        ))?;
        let res = sock.recv()?;
        match res {
            crate::protocol::Response::Decrypt { plaintext } => {
                Ok(plaintext)
            }
            crate::protocol::Response::Error { error } => {
                Err(anyhow::anyhow!("failed to decrypt: {error}"))
            }
            _ => Err(anyhow::anyhow!("unexpected message: {res:?}")),
        }
    }

    fn encrypt(
        &self,
        plaintext: &str,
        org_id: Option<&str>,
    ) -> anyhow::Result<String> {
        let mut sock = self.connect()?;
        sock.send(&crate::protocol::Request::new(
            self.environment.clone(),
            crate::protocol::Action::Encrypt {
                plaintext: plaintext.to_string(),
                org_id: org_id.map(std::string::ToString::to_string),
            },
        ))?;
        let res = sock.recv()?;
        match res {
            crate::protocol::Response::Encrypt { cipherstring } => {
                Ok(cipherstring)
            }
            crate::protocol::Response::Error { error } => {
                Err(anyhow::anyhow!("failed to encrypt: {error}"))
            }
            _ => Err(anyhow::anyhow!("unexpected message: {res:?}")),
        }
    }
}

impl AgentControl for AgentClient {
    fn simple_action(
        &self,
        action: crate::protocol::Action,
    ) -> anyhow::Result<()> {
        let mut sock = self.connect()?;
        sock.send(&crate::protocol::Request::new(
            self.environment.clone(),
            action,
        ))?;
        let res = sock.recv()?;
        match res {
            crate::protocol::Response::Ack => Ok(()),
            crate::protocol::Response::Error { error } => {
                Err(anyhow::anyhow!("{error}"))
            }
            _ => Err(anyhow::anyhow!("unexpected message: {res:?}")),
        }
    }

    fn version(&self) -> anyhow::Result<u32> {
        let mut sock = self.connect()?;
        sock.send(&crate::protocol::Request::new(
            self.environment.clone(),
            crate::protocol::Action::Version,
        ))?;
        let res = sock.recv()?;
        match res {
            crate::protocol::Response::Version { version } => Ok(version),
            crate::protocol::Response::Error { error } => {
                Err(anyhow::anyhow!("failed to get version: {error}"))
            }
            _ => Err(anyhow::anyhow!("unexpected message: {res:?}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Needle — search key for finding entries (name, URI, or UUID)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Needle {
    Name(String),
    Uri(url::Url),
    Uuid(uuid::Uuid, String),
}

impl std::fmt::Display for Needle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match &self {
            Self::Name(name) => name.clone(),
            Self::Uri(uri) => uri.to_string(),
            Self::Uuid(_, s) => s.clone(),
        };
        write!(f, "{value}")
    }
}

#[allow(clippy::unnecessary_wraps)]
pub fn parse_needle(arg: &str) -> Result<Needle, std::convert::Infallible> {
    if let Ok(uuid) = uuid::Uuid::parse_str(arg) {
        return Ok(Needle::Uuid(uuid, arg.to_string()));
    }
    if let Ok(url) = url::Url::parse(arg) {
        if url.is_special() {
            return Ok(Needle::Uri(url));
        }
    }
    Ok(Needle::Name(arg.to_string()))
}

// ---------------------------------------------------------------------------
// FieldName — identifies a field for logging decrypt failures
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldName {
    Notes,
    Username,
    Password,
    Totp,
    Uris,
    IdentityName,
    City,
    State,
    PostalCode,
    Country,
    Phone,
    Ssn,
    License,
    Passport,
    CardNumber,
    Expiration,
    ExpMonth,
    ExpYear,
    Cvv,
    Cardholder,
    Brand,
    Name,
    Email,
    Address,
    Address1,
    Address2,
    Address3,
    Fingerprint,
    PublicKey,
    PrivateKey,
    Title,
    FirstName,
    MiddleName,
    LastName,
}

impl FieldName {
    fn as_str(&self) -> &str {
        match self {
            Self::Notes => "notes",
            Self::Username => "username",
            Self::Password => "password",
            Self::Totp => "totp",
            Self::Uris => "uris",
            Self::IdentityName => "identityname",
            Self::City => "city",
            Self::State => "state",
            Self::PostalCode => "postcode",
            Self::Country => "country",
            Self::Phone => "phone",
            Self::Ssn => "ssn",
            Self::License => "license",
            Self::Passport => "passport",
            Self::CardNumber => "number",
            Self::Expiration => "exp",
            Self::ExpMonth => "exp_month",
            Self::ExpYear => "exp_year",
            Self::Cvv => "cvv",
            Self::Cardholder => "cardholder",
            Self::Brand => "brand",
            Self::Name => "name",
            Self::Email => "email",
            Self::Address1 => "address1",
            Self::Address2 => "address2",
            Self::Address3 => "address3",
            Self::Address => "address",
            Self::Fingerprint => "fingerprint",
            Self::PublicKey => "public_key",
            Self::PrivateKey => "private_key",
            Self::Title => "title",
            Self::FirstName => "first_name",
            Self::MiddleName => "middle_name",
            Self::LastName => "last_name",
        }
    }
}

impl std::str::FromStr for FieldName {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "notes" | "note" => Self::Notes,
            "username" | "user" => Self::Username,
            "password" => Self::Password,
            "totp" | "code" => Self::Totp,
            "uris" | "urls" | "sites" => Self::Uris,
            "identityname" => Self::IdentityName,
            "city" => Self::City,
            "state" => Self::State,
            "postcode" | "zipcode" | "zip" => Self::PostalCode,
            "country" => Self::Country,
            "phone" => Self::Phone,
            "ssn" => Self::Ssn,
            "license" => Self::License,
            "passport" => Self::Passport,
            "number" | "card" => Self::CardNumber,
            "exp" => Self::Expiration,
            "exp_month" | "month" => Self::ExpMonth,
            "exp_year" | "year" => Self::ExpYear,
            "cvv" => Self::Cvv,
            "cardholder" | "cardholder_name" => Self::Cardholder,
            "brand" | "type" => Self::Brand,
            "name" => Self::Name,
            "email" => Self::Email,
            "address1" => Self::Address1,
            "address2" => Self::Address2,
            "address3" => Self::Address3,
            "address" => Self::Address,
            "fingerprint" => Self::Fingerprint,
            "public_key" => Self::PublicKey,
            "private_key" => Self::PrivateKey,
            "title" => Self::Title,
            "first_name" => Self::FirstName,
            "middle_name" => Self::MiddleName,
            "last_name" => Self::LastName,
            _ => anyhow::bail!("unknown field {s}"),
        })
    }
}

impl std::fmt::Display for FieldName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SearchEntry — lightweight decrypted view for searching/matching
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct SearchEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub folder: Option<String>,
    pub name: String,
    pub user: Option<String>,
    pub uris: Vec<(String, Option<crate::api::UriMatchType>)>,
    pub fields: Vec<String>,
    pub notes: Option<String>,
}

impl SearchEntry {
    pub fn display_name(&self) -> String {
        self.user.as_ref().map_or_else(
            || self.name.clone(),
            |user| format!("{user}@{}", self.name),
        )
    }

    pub fn matches(
        &self,
        needle: &Needle,
        username: Option<&str>,
        folder: Option<&str>,
        ignore_case: bool,
        strict_username: bool,
        strict_folder: bool,
        exact: bool,
    ) -> bool {
        let match_str = match (ignore_case, exact) {
            (true, true) => |field: &str, search_term: &str| {
                field.to_lowercase() == search_term.to_lowercase()
            },
            (true, false) => |field: &str, search_term: &str| {
                field.to_lowercase().contains(&search_term.to_lowercase())
            },
            (false, true) => {
                |field: &str, search_term: &str| field == search_term
            }
            (false, false) => {
                |field: &str, search_term: &str| field.contains(search_term)
            }
        };

        match (self.folder.as_deref(), folder) {
            (Some(folder), Some(given_folder)) => {
                if !match_str(folder, given_folder) {
                    return false;
                }
            }
            (Some(_), None) => {
                if strict_folder {
                    return false;
                }
            }
            (None, Some(_)) => {
                return false;
            }
            (None, None) => {}
        }

        match (&self.user, username) {
            (Some(username), Some(given_username)) => {
                if !match_str(username, given_username) {
                    return false;
                }
            }
            (Some(_), None) => {
                if strict_username {
                    return false;
                }
            }
            (None, Some(_)) => {
                return false;
            }
            (None, None) => {}
        }

        match needle {
            Needle::Uuid(uuid, s) => {
                if uuid::Uuid::parse_str(&self.id) != Ok(*uuid)
                    && !match_str(&self.name, s)
                {
                    return false;
                }
            }
            Needle::Name(name) => {
                if !match_str(&self.name, name) {
                    return false;
                }
            }
            Needle::Uri(given_uri) => {
                if self.uris.iter().all(|(uri, match_type)| {
                    !matches_url(uri, *match_type, given_uri)
                }) {
                    return false;
                }
            }
        }

        true
    }

    pub fn search_match(&self, term: &str, folder: Option<&str>) -> bool {
        if let Some(folder) = folder {
            if self.folder.as_deref() != Some(folder) {
                return false;
            }
        }

        let mut fields = vec![self.name.clone()];
        if let Some(notes) = &self.notes {
            fields.push(notes.clone());
        }
        if let Some(user) = &self.user {
            fields.push(user.clone());
        }
        fields.extend(self.uris.iter().map(|(uri, _)| uri).cloned());
        fields.extend(self.fields.iter().cloned());

        for field in fields {
            if field.to_lowercase().contains(&term.to_lowercase()) {
                return true;
            }
        }

        false
    }
}

// ---------------------------------------------------------------------------
// URL matching helpers
// ---------------------------------------------------------------------------

pub fn matches_url(
    url: &str,
    match_type: Option<crate::api::UriMatchType>,
    given_url: &url::Url,
) -> bool {
    match match_type.unwrap_or(crate::api::UriMatchType::Domain) {
        crate::api::UriMatchType::Domain => {
            let Some(given_host_port) = host_port(given_url) else {
                return false;
            };
            if let Ok(self_url) = url::Url::parse(url) {
                if let Some(self_host_port) = host_port(&self_url) {
                    if self_url.scheme() == given_url.scheme()
                        && (self_host_port == given_host_port
                            || given_host_port
                                .ends_with(&format!(".{self_host_port}")))
                    {
                        return true;
                    }
                }
            }
            url == given_host_port
                || given_host_port.ends_with(&format!(".{url}"))
        }
        crate::api::UriMatchType::Host => {
            let Some(given_host_port) = host_port(given_url) else {
                return false;
            };
            if let Ok(self_url) = url::Url::parse(url) {
                if let Some(self_host_port) = host_port(&self_url) {
                    if self_url.scheme() == given_url.scheme()
                        && self_host_port == given_host_port
                    {
                        return true;
                    }
                }
            }
            url == given_host_port
        }
        crate::api::UriMatchType::StartsWith => {
            given_url.to_string().starts_with(url)
        }
        crate::api::UriMatchType::Exact => {
            if given_url.path() == "/" {
                given_url.to_string().trim_end_matches('/')
                    == url.trim_end_matches('/')
            } else {
                given_url.to_string() == url
            }
        }
        crate::api::UriMatchType::RegularExpression => {
            let Ok(rx) = regex::Regex::new(url) else {
                return false;
            };
            rx.is_match(given_url.as_ref())
        }
        crate::api::UriMatchType::Never => false,
    }
}

fn host_port(url: &url::Url) -> Option<String> {
    let host = url.host_str()?;
    Some(
        url.port().map_or_else(
            || host.to_string(),
            |port| format!("{host}:{port}"),
        ),
    )
}

// ---------------------------------------------------------------------------
// find_entry_raw — pure search logic over pre-decrypted entries
// ---------------------------------------------------------------------------

pub fn find_entry_raw(
    entries: &[(crate::db::Entry, SearchEntry)],
    needle: &Needle,
    username: Option<&str>,
    folder: Option<&str>,
    ignore_case: bool,
) -> anyhow::Result<(crate::db::Entry, SearchEntry)> {
    let mut matches: Vec<(crate::db::Entry, SearchEntry)> = vec![];

    let find_matches = |strict_username, strict_folder, exact| {
        entries
            .iter()
            .filter(|&(_, search)| {
                search.matches(
                    needle,
                    username,
                    folder,
                    ignore_case,
                    strict_username,
                    strict_folder,
                    exact,
                )
            })
            .cloned()
            .collect()
    };

    for exact in [true, false] {
        matches = find_matches(true, true, exact);
        if matches.len() == 1 {
            return Ok(matches[0].clone());
        }

        let strict_folder_matches = find_matches(false, true, exact);
        let strict_username_matches = find_matches(true, false, exact);
        if strict_folder_matches.len() == 1
            && strict_username_matches.len() != 1
        {
            return Ok(strict_folder_matches[0].clone());
        } else if strict_folder_matches.len() != 1
            && strict_username_matches.len() == 1
        {
            return Ok(strict_username_matches[0].clone());
        }

        matches = find_matches(false, false, exact);
        if matches.len() == 1 {
            return Ok(matches[0].clone());
        }
    }

    if matches.is_empty() {
        Err(anyhow::anyhow!("no entry found"))
    } else {
        let entries: Vec<String> = matches
            .iter()
            .map(|(_, search)| search.display_name())
            .collect();
        let entries = entries.join(", ");
        Err(anyhow::anyhow!("multiple entries found: {entries}"))
    }
}

// ---------------------------------------------------------------------------
// EntryDraft — plaintext data for creating or editing an entry
// ---------------------------------------------------------------------------

/// Flat plaintext representation for creating/editing vault entries.
///
/// All text fields are plain `String` — empty string means "not set".
/// Conversion to `Option`/`Data` happens at the API boundary.
#[derive(Debug, Clone, Default)]
pub struct EntryDraft {
    pub name: String,
    pub username: String,
    pub password: String,
    pub totp: String,
    pub uris: Vec<String>,
    pub folder: String,
    pub notes: String,
    pub org_id: Option<String>,
}

impl EntryDraft {
    /// Removes empty URI strings from the list.
    pub fn clean_uris(&mut self) {
        self.uris.retain(|u| !u.is_empty());
    }

    /// Converts to the `decrypted::Data` representation used by the API layer.
    pub fn to_data(&self) -> crate::decrypted::Data {
        let none_if_empty = |s: &str| -> Option<String> {
            if s.is_empty() { None } else { Some(s.to_string()) }
        };
        let uris: Vec<crate::decrypted::Uri> = self
            .uris
            .iter()
            .filter(|u| !u.is_empty())
            .map(|u| crate::decrypted::Uri {
                uri: u.clone(),
                match_type: None,
            })
            .collect();
        crate::decrypted::Data::Login {
            username: none_if_empty(&self.username),
            password: none_if_empty(&self.password),
            totp: none_if_empty(&self.totp),
            uris: if uris.is_empty() { None } else { Some(uris) },
        }
    }

    /// Converts the folder to `Option<&str>` for the API layer.
    pub fn folder_option(&self) -> Option<&str> {
        if self.folder.is_empty() { None } else { Some(&self.folder) }
    }

    /// Converts notes to `Option<&str>` for the API layer.
    pub fn notes_option(&self) -> Option<&str> {
        if self.notes.is_empty() { None } else { Some(&self.notes) }
    }
}

// ---------------------------------------------------------------------------
// Client — high-level orchestrator over CryptoProvider
// ---------------------------------------------------------------------------

pub struct Client<C: CryptoProvider> {
    crypto: C,
}

impl<C: CryptoProvider> Client<C> {
    pub fn new(crypto: C) -> Self {
        Self { crypto }
    }

    pub fn decrypt(
        &self,
        cipherstring: &str,
        entry_key: Option<&str>,
        org_id: Option<&str>,
    ) -> anyhow::Result<String> {
        self.crypto.decrypt(cipherstring, entry_key, org_id)
    }

    // -- DB helpers --

    pub fn load_db(&self) -> anyhow::Result<crate::db::Db> {
        let config = crate::config::Config::load()?;
        config.email.as_ref().map_or_else(
            || {
                Err(anyhow::anyhow!(
                    "failed to find email address in config"
                ))
            },
            |email| {
                crate::db::Db::load(&config.server_name(), email)
                    .map_err(anyhow::Error::new)
            },
        )
    }

    pub fn save_db(&self, db: &crate::db::Db) -> anyhow::Result<()> {
        let config = crate::config::Config::load()?;
        config.email.as_ref().map_or_else(
            || {
                Err(anyhow::anyhow!(
                    "failed to find email address in config"
                ))
            },
            |email| {
                db.save(&config.server_name(), email)
                    .map_err(anyhow::Error::new)
            },
        )
    }

    // -- Encrypt helpers --

    pub fn encrypt_string(
        &self,
        plaintext: &str,
        org_id: Option<&str>,
    ) -> anyhow::Result<String> {
        self.crypto.encrypt(plaintext, org_id)
    }

    pub fn encrypt_optional_string(
        &self,
        value: Option<&str>,
        org_id: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        value
            .map(|value| self.crypto.encrypt(value, org_id))
            .transpose()
    }

    pub fn encrypt_entry_data(
        &self,
        data: &crate::decrypted::Data,
        org_id: Option<&str>,
    ) -> anyhow::Result<crate::db::EntryData> {
        match data {
            crate::decrypted::Data::Login {
                username,
                password,
                totp,
                uris,
            } => Ok(crate::db::EntryData::Login {
                username: self
                    .encrypt_optional_string(username.as_deref(), org_id)?,
                password: self
                    .encrypt_optional_string(password.as_deref(), org_id)?,
                totp: self
                    .encrypt_optional_string(totp.as_deref(), org_id)?,
                uris: uris
                    .as_ref()
                    .map(|uris| {
                        uris.iter()
                            .map(|uri| {
                                Ok(crate::db::Uri {
                                    uri: self
                                        .crypto
                                        .encrypt(&uri.uri, org_id)?,
                                    match_type: uri.match_type,
                                })
                            })
                            .collect::<anyhow::Result<Vec<_>>>()
                    })
                    .transpose()?
                    .unwrap_or_default(),
            }),
            crate::decrypted::Data::Card {
                cardholder_name,
                number,
                brand,
                exp_month,
                exp_year,
                code,
            } => Ok(crate::db::EntryData::Card {
                cardholder_name: self.encrypt_optional_string(
                    cardholder_name.as_deref(),
                    org_id,
                )?,
                number: self
                    .encrypt_optional_string(number.as_deref(), org_id)?,
                brand: self
                    .encrypt_optional_string(brand.as_deref(), org_id)?,
                exp_month: self
                    .encrypt_optional_string(exp_month.as_deref(), org_id)?,
                exp_year: self
                    .encrypt_optional_string(exp_year.as_deref(), org_id)?,
                code: self
                    .encrypt_optional_string(code.as_deref(), org_id)?,
            }),
            crate::decrypted::Data::Identity {
                title,
                first_name,
                middle_name,
                last_name,
                address1,
                address2,
                address3,
                city,
                state,
                postal_code,
                country,
                phone,
                email,
                ssn,
                license_number,
                passport_number,
                username,
            } => Ok(crate::db::EntryData::Identity {
                title: self
                    .encrypt_optional_string(title.as_deref(), org_id)?,
                first_name: self
                    .encrypt_optional_string(first_name.as_deref(), org_id)?,
                middle_name: self.encrypt_optional_string(
                    middle_name.as_deref(),
                    org_id,
                )?,
                last_name: self
                    .encrypt_optional_string(last_name.as_deref(), org_id)?,
                address1: self
                    .encrypt_optional_string(address1.as_deref(), org_id)?,
                address2: self
                    .encrypt_optional_string(address2.as_deref(), org_id)?,
                address3: self
                    .encrypt_optional_string(address3.as_deref(), org_id)?,
                city: self
                    .encrypt_optional_string(city.as_deref(), org_id)?,
                state: self
                    .encrypt_optional_string(state.as_deref(), org_id)?,
                postal_code: self.encrypt_optional_string(
                    postal_code.as_deref(),
                    org_id,
                )?,
                country: self
                    .encrypt_optional_string(country.as_deref(), org_id)?,
                phone: self
                    .encrypt_optional_string(phone.as_deref(), org_id)?,
                email: self
                    .encrypt_optional_string(email.as_deref(), org_id)?,
                ssn: self
                    .encrypt_optional_string(ssn.as_deref(), org_id)?,
                license_number: self.encrypt_optional_string(
                    license_number.as_deref(),
                    org_id,
                )?,
                passport_number: self.encrypt_optional_string(
                    passport_number.as_deref(),
                    org_id,
                )?,
                username: self
                    .encrypt_optional_string(username.as_deref(), org_id)?,
            }),
            crate::decrypted::Data::SecureNote => {
                Ok(crate::db::EntryData::SecureNote)
            }
            crate::decrypted::Data::SshKey {
                public_key,
                fingerprint,
                private_key,
            } => Ok(crate::db::EntryData::SshKey {
                public_key: self
                    .encrypt_optional_string(public_key.as_deref(), org_id)?,
                fingerprint: self.encrypt_optional_string(
                    fingerprint.as_deref(),
                    org_id,
                )?,
                private_key: self.encrypt_optional_string(
                    private_key.as_deref(),
                    org_id,
                )?,
            }),
        }
    }

    // -- Decrypt helpers --

    fn decrypt_field(
        &self,
        name: FieldName,
        field: Option<&str>,
        entry_key: Option<&str>,
        org_id: Option<&str>,
    ) -> Option<String> {
        let field = field
            .map(|field| self.crypto.decrypt(field, entry_key, org_id))
            .transpose();
        match field {
            Ok(field) => field,
            Err(e) => {
                log::warn!("failed to decrypt {name}: {e}");
                None
            }
        }
    }

    pub fn decrypt_entry(
        &self,
        entry: &crate::db::Entry,
    ) -> anyhow::Result<crate::decrypted::Cipher> {
        // folder name should always be decrypted with the local key
        let folder = entry
            .folder
            .as_ref()
            .map(|folder| self.crypto.decrypt(folder, None, None))
            .transpose();
        let folder = match folder {
            Ok(folder) => folder,
            Err(e) => {
                log::warn!("failed to decrypt folder name: {e}");
                None
            }
        };
        let fields = entry
            .fields
            .iter()
            .map(|field| {
                Ok(crate::decrypted::Field {
                    name: field
                        .name
                        .as_ref()
                        .map(|name| {
                            self.crypto.decrypt(
                                name,
                                entry.key.as_deref(),
                                entry.org_id.as_deref(),
                            )
                        })
                        .transpose()?,
                    value: field
                        .value
                        .as_ref()
                        .map(|value| {
                            self.crypto.decrypt(
                                value,
                                entry.key.as_deref(),
                                entry.org_id.as_deref(),
                            )
                        })
                        .transpose()?,
                    ty: field.ty,
                })
            })
            .collect::<anyhow::Result<_>>()?;
        let notes = entry
            .notes
            .as_ref()
            .map(|notes| {
                self.crypto.decrypt(
                    notes,
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                )
            })
            .transpose();
        let notes = match notes {
            Ok(notes) => notes,
            Err(e) => {
                log::warn!("failed to decrypt notes: {e}");
                None
            }
        };
        let history = entry
            .history
            .iter()
            .map(|history_entry| {
                Ok(crate::decrypted::HistoryEntry {
                    last_used_date: history_entry.last_used_date.clone(),
                    password: self.crypto.decrypt(
                        &history_entry.password,
                        entry.key.as_deref(),
                        entry.org_id.as_deref(),
                    )?,
                })
            })
            .collect::<anyhow::Result<_>>()?;

        let data = match &entry.data {
            crate::db::EntryData::Login {
                username,
                password,
                totp,
                uris,
            } => crate::decrypted::Data::Login {
                username: self.decrypt_field(
                    FieldName::Username,
                    username.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                password: self.decrypt_field(
                    FieldName::Password,
                    password.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                totp: self.decrypt_field(
                    FieldName::Totp,
                    totp.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                uris: uris
                    .iter()
                    .map(|s| {
                        self.decrypt_field(
                            FieldName::Uris,
                            Some(&s.uri),
                            entry.key.as_deref(),
                            entry.org_id.as_deref(),
                        )
                        .map(|uri| crate::decrypted::Uri {
                            uri,
                            match_type: s.match_type,
                        })
                    })
                    .collect(),
            },
            crate::db::EntryData::Card {
                cardholder_name,
                number,
                brand,
                exp_month,
                exp_year,
                code,
            } => crate::decrypted::Data::Card {
                cardholder_name: self.decrypt_field(
                    FieldName::Cardholder,
                    cardholder_name.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                number: self.decrypt_field(
                    FieldName::CardNumber,
                    number.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                brand: self.decrypt_field(
                    FieldName::Brand,
                    brand.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                exp_month: self.decrypt_field(
                    FieldName::ExpMonth,
                    exp_month.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                exp_year: self.decrypt_field(
                    FieldName::ExpYear,
                    exp_year.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                code: self.decrypt_field(
                    FieldName::Cvv,
                    code.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
            },
            crate::db::EntryData::Identity {
                title,
                first_name,
                middle_name,
                last_name,
                address1,
                address2,
                address3,
                city,
                state,
                postal_code,
                country,
                phone,
                email,
                ssn,
                license_number,
                passport_number,
                username,
            } => crate::decrypted::Data::Identity {
                title: self.decrypt_field(
                    FieldName::Title,
                    title.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                first_name: self.decrypt_field(
                    FieldName::FirstName,
                    first_name.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                middle_name: self.decrypt_field(
                    FieldName::MiddleName,
                    middle_name.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                last_name: self.decrypt_field(
                    FieldName::LastName,
                    last_name.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                address1: self.decrypt_field(
                    FieldName::Address1,
                    address1.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                address2: self.decrypt_field(
                    FieldName::Address2,
                    address2.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                address3: self.decrypt_field(
                    FieldName::Address3,
                    address3.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                city: self.decrypt_field(
                    FieldName::City,
                    city.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                state: self.decrypt_field(
                    FieldName::State,
                    state.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                postal_code: self.decrypt_field(
                    FieldName::PostalCode,
                    postal_code.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                country: self.decrypt_field(
                    FieldName::Country,
                    country.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                phone: self.decrypt_field(
                    FieldName::Phone,
                    phone.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                email: self.decrypt_field(
                    FieldName::Email,
                    email.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                ssn: self.decrypt_field(
                    FieldName::Ssn,
                    ssn.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                license_number: self.decrypt_field(
                    FieldName::License,
                    license_number.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                passport_number: self.decrypt_field(
                    FieldName::Passport,
                    passport_number.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                username: self.decrypt_field(
                    FieldName::Username,
                    username.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
            },
            crate::db::EntryData::SecureNote => {
                crate::decrypted::Data::SecureNote
            }
            crate::db::EntryData::SshKey {
                public_key,
                fingerprint,
                private_key,
            } => crate::decrypted::Data::SshKey {
                public_key: self.decrypt_field(
                    FieldName::PublicKey,
                    public_key.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                fingerprint: self.decrypt_field(
                    FieldName::Fingerprint,
                    fingerprint.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
                private_key: self.decrypt_field(
                    FieldName::PrivateKey,
                    private_key.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
            },
        };

        Ok(crate::decrypted::Cipher {
            id: entry.id.clone(),
            folder,
            name: self.crypto.decrypt(
                &entry.name,
                entry.key.as_deref(),
                entry.org_id.as_deref(),
            )?,
            data,
            fields,
            notes,
            history,
        })
    }

    pub fn decrypt_search_entry(
        &self,
        entry: &crate::db::Entry,
    ) -> anyhow::Result<SearchEntry> {
        let id = entry.id.clone();
        let name = self.crypto.decrypt(
            &entry.name,
            entry.key.as_deref(),
            entry.org_id.as_deref(),
        )?;
        let user = match &entry.data {
            crate::db::EntryData::Login { username, .. } => self
                .decrypt_field(
                    FieldName::Username,
                    username.as_deref(),
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                ),
            _ => None,
        };
        // folder name should always be decrypted with the local key
        let folder = entry
            .folder
            .as_ref()
            .map(|folder| self.crypto.decrypt(folder, None, None))
            .transpose()?;
        let notes = entry
            .notes
            .as_ref()
            .map(|notes| {
                self.crypto.decrypt(
                    notes,
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                )
            })
            .transpose();
        let uris =
            if let crate::db::EntryData::Login { uris, .. } = &entry.data {
                uris.iter()
                    .filter_map(|s| {
                        self.decrypt_field(
                            FieldName::Uris,
                            Some(&s.uri),
                            entry.key.as_deref(),
                            entry.org_id.as_deref(),
                        )
                        .map(|uri| (uri, s.match_type))
                    })
                    .collect()
            } else {
                vec![]
            };
        let fields = entry
            .fields
            .iter()
            .filter_map(|field| {
                if field.ty == Some(crate::api::FieldType::Hidden) {
                    None
                } else {
                    field.value.as_ref()
                }
            })
            .map(|value| {
                self.crypto.decrypt(
                    value,
                    entry.key.as_deref(),
                    entry.org_id.as_deref(),
                )
            })
            .collect::<anyhow::Result<_>>()?;
        let notes = match notes {
            Ok(notes) => notes,
            Err(e) => {
                log::warn!("failed to decrypt notes: {e}");
                None
            }
        };
        let entry_type = (match &entry.data {
            crate::db::EntryData::Login { .. } => "Login",
            crate::db::EntryData::Identity { .. } => "Identity",
            crate::db::EntryData::SshKey { .. } => "SSH Key",
            crate::db::EntryData::SecureNote => "Note",
            crate::db::EntryData::Card { .. } => "Card",
        })
        .to_string();

        Ok(SearchEntry {
            id,
            entry_type,
            folder,
            name,
            user,
            uris,
            fields,
            notes,
        })
    }

    // -- High-level: find --

    pub fn find_entry(
        &self,
        db: &crate::db::Db,
        mut needle: Needle,
        username: Option<&str>,
        folder: Option<&str>,
        ignore_case: bool,
    ) -> anyhow::Result<(crate::db::Entry, crate::decrypted::Cipher)> {
        if let Needle::Uuid(uuid, s) = needle {
            for cipher in &db.entries {
                if uuid::Uuid::parse_str(&cipher.id) == Ok(uuid) {
                    return Ok((
                        cipher.clone(),
                        self.decrypt_entry(cipher)?,
                    ));
                }
            }
            needle = Needle::Name(s);
        }

        let ciphers: Vec<(crate::db::Entry, SearchEntry)> = db
            .entries
            .iter()
            .map(|entry| {
                self.decrypt_search_entry(entry)
                    .map(|decrypted| (entry.clone(), decrypted))
            })
            .collect::<anyhow::Result<_>>()?;
        let (entry, _) =
            find_entry_raw(&ciphers, &needle, username, folder, ignore_case)?;
        let decrypted_entry = self.decrypt_entry(&entry)?;
        Ok((entry, decrypted_entry))
    }

    // -- High-level: list --

    pub fn list_entries(
        &self,
    ) -> anyhow::Result<Vec<crate::decrypted::Cipher>> {
        let db = self.load_db()?;
        let mut entries: Vec<crate::decrypted::Cipher> = db
            .entries
            .iter()
            .map(|entry| self.decrypt_entry(entry))
            .collect::<anyhow::Result<_>>()?;
        entries.sort_unstable_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(entries)
    }

    // -- High-level: add --

    pub fn add_entry(
        &self,
        draft: &EntryDraft,
    ) -> anyhow::Result<()> {
        let mut db = self.load_db()?;
        let access_token = db
            .access_token
            .as_ref()
            .context("not logged in")?
            .clone();
        let refresh_token =
            db.refresh_token.as_ref().context("not logged in")?;

        let org_id = draft.org_id.as_deref();
        let name = self.crypto.encrypt(&draft.name, org_id)?;
        let notes =
            self.encrypt_optional_string(draft.notes_option(), org_id)?;
        let data = self.encrypt_entry_data(&draft.to_data(), org_id)?;

        let mut folder_id = None;
        if let Some(folder_name) = draft.folder_option() {
            let (new_access_token, folders) =
                crate::actions::list_folders(&access_token, refresh_token)?;
            if let Some(new_access_token) = new_access_token {
                db.access_token = Some(new_access_token);
                self.save_db(&db)?;
            }
            let access_token =
                db.access_token.as_ref().context("not logged in")?;

            let folders: Vec<(String, String)> = folders
                .iter()
                .cloned()
                .map(|(id, name)| {
                    Ok((id, self.crypto.decrypt(&name, None, None)?))
                })
                .collect::<anyhow::Result<_>>()?;

            for (id, name) in folders {
                if name == folder_name {
                    folder_id = Some(id);
                }
            }
            if folder_id.is_none() {
                let (new_access_token, id) =
                    crate::actions::create_folder(
                        access_token,
                        refresh_token,
                        &self.crypto.encrypt(folder_name, None)?,
                    )?;
                if let Some(new_access_token) = new_access_token {
                    db.access_token = Some(new_access_token);
                    self.save_db(&db)?;
                }
                folder_id = Some(id);
            }
        }

        let access_token =
            db.access_token.as_ref().context("not logged in")?;
        if let (Some(new_access_token), ()) = crate::actions::add(
            access_token,
            refresh_token,
            &name,
            &data,
            notes.as_deref(),
            folder_id.as_deref(),
        )? {
            db.access_token = Some(new_access_token);
            self.save_db(&db)?;
        }

        Ok(())
    }

    // -- High-level: edit --

    pub fn edit_entry(
        &self,
        entry: &crate::db::Entry,
        data: &crate::decrypted::Data,
        notes: Option<&str>,
        history: &[crate::db::HistoryEntry],
    ) -> anyhow::Result<()> {
        let mut db = self.load_db()?;
        let access_token = db
            .access_token
            .as_ref()
            .context("not logged in")?;
        let refresh_token =
            db.refresh_token.as_ref().context("not logged in")?;

        let org_id = entry.org_id.as_deref();
        let encrypted_data = self.encrypt_entry_data(data, org_id)?;
        let encrypted_notes =
            self.encrypt_optional_string(notes, org_id)?;

        if let (Some(new_access_token), ()) = crate::actions::edit(
            access_token,
            refresh_token,
            &entry.id,
            org_id,
            &entry.name,
            &encrypted_data,
            &entry.fields,
            encrypted_notes.as_deref(),
            entry.folder_id.as_deref(),
            history,
        )? {
            db.access_token = Some(new_access_token);
            self.save_db(&db)?;
        }

        Ok(())
    }

    // -- High-level: remove --

    pub fn remove_entry(&self, id: &str) -> anyhow::Result<()> {
        let mut db = self.load_db()?;
        let access_token = db
            .access_token
            .as_ref()
            .context("not logged in")?;
        let refresh_token =
            db.refresh_token.as_ref().context("not logged in")?;

        if let (Some(new_access_token), ()) =
            crate::actions::remove(access_token, refresh_token, id)?
        {
            db.access_token = Some(new_access_token);
        }
        db.entries.retain(|e| e.id != id);
        self.save_db(&db)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Agent process management
// ---------------------------------------------------------------------------

/// Starts the rbw-agent process. Respects `RBW_AGENT` env var for the binary path.
pub fn run_agent() -> anyhow::Result<()> {
    let agent_path = std::env::var_os("RBW_AGENT");
    let agent_path = agent_path.as_deref().unwrap_or_else(|| {
        use std::os::unix::ffi::OsStrExt as _;
        std::ffi::OsStr::from_bytes(b"rbw-agent")
    });
    let status = std::process::Command::new(agent_path)
        .status()
        .context("failed to run rbw-agent")?;
    if !status.success() {
        if let Some(code) = status.code() {
            // exit code 23 means the agent is already running
            if code != 23 {
                anyhow::bail!("failed to run rbw-agent: {status}");
            }
        }
    }
    Ok(())
}

// Agent-control methods only available when the crypto provider also
// implements AgentControl (i.e., the real AgentClient).
impl<C: CryptoProvider + AgentControl> Client<C> {
    pub fn sync(&self) -> anyhow::Result<()> {
        self.crypto
            .simple_action(crate::protocol::Action::Sync)
    }

    pub fn login(&self) -> anyhow::Result<()> {
        self.crypto
            .simple_action(crate::protocol::Action::Login)
    }

    pub fn unlock(&self) -> anyhow::Result<()> {
        self.crypto
            .simple_action(crate::protocol::Action::Unlock)
    }

    pub fn lock(&self) -> anyhow::Result<()> {
        self.crypto
            .simple_action(crate::protocol::Action::Lock)
    }

    pub fn check_lock(&self) -> anyhow::Result<()> {
        self.crypto
            .simple_action(crate::protocol::Action::CheckLock)
    }

    pub fn clipboard_store(&self, text: &str) -> anyhow::Result<()> {
        self.crypto.simple_action(
            crate::protocol::Action::ClipboardStore {
                text: text.to_string(),
            },
        )
    }

    pub fn version(&self) -> anyhow::Result<u32> {
        self.crypto.version()
    }

    /// Ensures the agent is running, starting it if necessary.
    /// Returns `Ok(())` if the agent is reachable after this call.
    pub fn ensure_agent(&self) -> anyhow::Result<()> {
        if self.version().is_ok() {
            return Ok(());
        }
        run_agent()?;
        self.version().map(|_| ())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCrypto;

    impl CryptoProvider for MockCrypto {
        fn decrypt(
            &self,
            cipherstring: &str,
            _entry_key: Option<&str>,
            _org_id: Option<&str>,
        ) -> anyhow::Result<String> {
            Ok(cipherstring
                .strip_prefix("enc:")
                .unwrap_or(cipherstring)
                .to_string())
        }

        fn encrypt(
            &self,
            plaintext: &str,
            _org_id: Option<&str>,
        ) -> anyhow::Result<String> {
            Ok(format!("enc:{plaintext}"))
        }
    }

    fn mock_client() -> Client<MockCrypto> {
        Client::new(MockCrypto)
    }

    fn make_login_db_entry(
        id: &str,
        name: &str,
        username: Option<&str>,
        uris: &[(&str, Option<crate::api::UriMatchType>)],
    ) -> crate::db::Entry {
        crate::db::Entry {
            id: id.to_string(),
            org_id: None,
            folder: None,
            folder_id: None,
            name: format!("enc:{name}"),
            data: crate::db::EntryData::Login {
                username: username.map(|u| format!("enc:{u}")),
                password: None,
                totp: None,
                uris: uris
                    .iter()
                    .map(|(uri, mt)| crate::db::Uri {
                        uri: format!("enc:{uri}"),
                        match_type: *mt,
                    })
                    .collect(),
            },
            fields: Vec::new(),
            notes: None,
            history: Vec::new(),
            key: None,
            master_password_reprompt:
                crate::api::CipherRepromptType::None,
        }
    }

    #[test]
    fn test_decrypt_entry_login() {
        let client = mock_client();
        let entry = make_login_db_entry(
            "id1",
            "GitHub",
            Some("user@example.com"),
            &[("https://github.com", None)],
        );
        let decrypted = client.decrypt_entry(&entry).unwrap();
        assert_eq!(decrypted.name, "GitHub");
        assert_eq!(decrypted.id, "id1");
        match &decrypted.data {
            crate::decrypted::Data::Login {
                username, uris, ..
            } => {
                assert_eq!(
                    username.as_deref(),
                    Some("user@example.com")
                );
                let uris = uris.as_ref().unwrap();
                assert_eq!(uris.len(), 1);
                assert_eq!(uris[0].uri, "https://github.com");
            }
            _ => panic!("expected Login data"),
        }
    }

    #[test]
    fn test_encrypt_entry_data_login() {
        let client = mock_client();
        let data = crate::decrypted::Data::Login {
            username: Some("alice".to_string()),
            password: Some("secret".to_string()),
            totp: None,
            uris: Some(vec![crate::decrypted::Uri {
                uri: "https://example.com".to_string(),
                match_type: Some(crate::api::UriMatchType::Domain),
            }]),
        };
        let encrypted = client.encrypt_entry_data(&data, None).unwrap();
        match &encrypted {
            crate::db::EntryData::Login {
                username,
                password,
                totp,
                uris,
            } => {
                assert_eq!(username.as_deref(), Some("enc:alice"));
                assert_eq!(password.as_deref(), Some("enc:secret"));
                assert_eq!(*totp, None);
                assert_eq!(uris.len(), 1);
                assert_eq!(uris[0].uri, "enc:https://example.com");
                assert_eq!(
                    uris[0].match_type,
                    Some(crate::api::UriMatchType::Domain)
                );
            }
            _ => panic!("expected Login data"),
        }
    }

    #[test]
    fn test_encrypt_optional_string() {
        let client = mock_client();
        assert_eq!(
            client.encrypt_optional_string(Some("hello"), None).unwrap(),
            Some("enc:hello".to_string())
        );
        assert_eq!(
            client.encrypt_optional_string(None, None).unwrap(),
            None
        );
    }

    #[test]
    fn test_decrypt_search_entry() {
        let client = mock_client();
        let entry = make_login_db_entry(
            "id1",
            "GitHub",
            Some("alice"),
            &[("https://github.com", None)],
        );
        let search = client.decrypt_search_entry(&entry).unwrap();
        assert_eq!(search.name, "GitHub");
        assert_eq!(search.user.as_deref(), Some("alice"));
        assert_eq!(search.entry_type, "Login");
        assert_eq!(search.uris.len(), 1);
        assert_eq!(search.uris[0].0, "https://github.com");
    }

    // -----------------------------------------------------------------------
    // Tests moved from commands.rs -- find_entry_raw and URL matching
    // -----------------------------------------------------------------------

    fn make_entry(
        name: &str,
        username: Option<&str>,
        folder: Option<&str>,
        uris: &[(&str, Option<crate::api::UriMatchType>)],
    ) -> (crate::db::Entry, SearchEntry) {
        let id = uuid::Uuid::new_v4();
        let db_entry = crate::db::Entry {
            id: id.to_string(),
            org_id: None,
            folder: folder
                .map(|_| "encrypted folder name".to_string()),
            folder_id: None,
            name: "this is the encrypted name".to_string(),
            data: crate::db::EntryData::Login {
                username: username.map(|_| {
                    "this is the encrypted username".to_string()
                }),
                password: None,
                totp: None,
                uris: uris
                    .iter()
                    .map(|(_, match_type)| crate::db::Uri {
                        uri: "this is the encrypted uri".to_string(),
                        match_type: *match_type,
                    })
                    .collect(),
            },
            fields: Vec::new(),
            notes: None,
            history: Vec::new(),
            key: None,
            master_password_reprompt:
                crate::api::CipherRepromptType::None,
        };
        let search = SearchEntry {
            id: id.to_string(),
            entry_type: "Login".to_string(),
            folder: folder.map(str::to_string),
            name: name.to_string(),
            user: username.map(str::to_string),
            uris: uris
                .iter()
                .map(|(uri, match_type)| {
                    ((*uri).to_string(), *match_type)
                })
                .collect(),
            fields: Vec::new(),
            notes: None,
        };
        (db_entry, search)
    }

    fn entries_eq(
        a: &(crate::db::Entry, SearchEntry),
        b: &(crate::db::Entry, SearchEntry),
    ) -> bool {
        a.0 == b.0 && a.1 == b.1
    }

    #[track_caller]
    fn one_match(
        entries: &[(crate::db::Entry, SearchEntry)],
        needle: &str,
        username: Option<&str>,
        folder: Option<&str>,
        idx: usize,
        ignore_case: bool,
    ) {
        let result = find_entry_raw(
            entries,
            &parse_needle(needle).unwrap(),
            username,
            folder,
            ignore_case,
        )
        .unwrap();
        assert!(
            entries_eq(&result, &entries[idx]),
            "expected {}, got {}",
            entries[idx].1.display_name(),
            result.1.display_name()
        );
    }

    #[track_caller]
    fn no_matches(
        entries: &[(crate::db::Entry, SearchEntry)],
        needle: &str,
        username: Option<&str>,
        folder: Option<&str>,
        ignore_case: bool,
    ) {
        let result = find_entry_raw(
            entries,
            &parse_needle(needle).unwrap(),
            username,
            folder,
            ignore_case,
        );
        assert!(
            result.as_ref().is_err_and(|e| e
                .to_string()
                .starts_with("no entry found")),
            "expected no match, got {result:?}"
        );
    }

    #[track_caller]
    fn many_matches(
        entries: &[(crate::db::Entry, SearchEntry)],
        needle: &str,
        username: Option<&str>,
        folder: Option<&str>,
        ignore_case: bool,
    ) {
        let result = find_entry_raw(
            entries,
            &parse_needle(needle).unwrap(),
            username,
            folder,
            ignore_case,
        );
        assert!(
            result.as_ref().is_err_and(|e| e
                .to_string()
                .starts_with("multiple entries found")),
            "expected multiple matches, got {result:?}"
        );
    }

    #[test]
    fn test_find_entry() {
        let entries = &[
            make_entry("github", Some("foo"), None, &[]),
            make_entry("gitlab", Some("foo"), None, &[]),
            make_entry("gitlab", Some("bar"), None, &[]),
            make_entry("gitter", Some("baz"), None, &[]),
            make_entry("git", Some("foo"), None, &[]),
            make_entry("bitwarden", None, None, &[]),
            make_entry("github", Some("foo"), Some("websites"), &[]),
            make_entry("github", Some("foo"), Some("ssh"), &[]),
            make_entry("github", Some("root"), Some("ssh"), &[]),
            make_entry("codeberg", Some("foo"), None, &[]),
            make_entry("codeberg", None, None, &[]),
            make_entry("1password", Some("foo"), None, &[]),
            make_entry("1password", None, Some("foo"), &[]),
        ];

        one_match(entries, "github", Some("foo"), None, 0, false);
        one_match(entries, "GITHUB", Some("foo"), None, 0, true);
        one_match(entries, "github", None, None, 0, false);
        one_match(entries, "GITHUB", None, None, 0, true);
        one_match(entries, "gitlab", Some("foo"), None, 1, false);
        one_match(entries, "GITLAB", Some("foo"), None, 1, true);
        one_match(entries, "git", Some("bar"), None, 2, false);
        one_match(entries, "GIT", Some("bar"), None, 2, true);
        one_match(entries, "gitter", Some("ba"), None, 3, false);
        one_match(entries, "GITTER", Some("ba"), None, 3, true);
        one_match(entries, "git", Some("foo"), None, 4, false);
        one_match(entries, "GIT", Some("foo"), None, 4, true);
        one_match(entries, "git", None, None, 4, false);
        one_match(entries, "GIT", None, None, 4, true);
        one_match(entries, "bitwarden", None, None, 5, false);
        one_match(entries, "BITWARDEN", None, None, 5, true);
        one_match(
            entries, "github", Some("foo"), Some("websites"), 6, false,
        );
        one_match(
            entries, "GITHUB", Some("foo"), Some("websites"), 6, true,
        );
        one_match(
            entries, "github", Some("foo"), Some("ssh"), 7, false,
        );
        one_match(
            entries, "GITHUB", Some("foo"), Some("ssh"), 7, true,
        );
        one_match(entries, "github", Some("root"), None, 8, false);
        one_match(entries, "GITHUB", Some("root"), None, 8, true);

        no_matches(entries, "gitlab", Some("baz"), None, false);
        no_matches(entries, "GITLAB", Some("baz"), None, true);
        no_matches(entries, "bitbucket", Some("foo"), None, false);
        no_matches(entries, "BITBUCKET", Some("foo"), None, true);
        no_matches(
            entries, "github", Some("foo"), Some("bar"), false,
        );
        no_matches(
            entries, "GITHUB", Some("foo"), Some("bar"), true,
        );
        no_matches(
            entries, "gitlab", Some("foo"), Some("bar"), false,
        );
        no_matches(
            entries, "GITLAB", Some("foo"), Some("bar"), true,
        );

        many_matches(entries, "gitlab", None, None, false);
        many_matches(entries, "gitlab", None, None, true);
        many_matches(entries, "gi", Some("foo"), None, false);
        many_matches(entries, "GI", Some("foo"), None, true);
        many_matches(entries, "git", Some("ba"), None, false);
        many_matches(entries, "GIT", Some("ba"), None, true);
        many_matches(
            entries, "github", Some("foo"), Some("s"), false,
        );
        many_matches(
            entries, "GITHUB", Some("foo"), Some("s"), true,
        );

        one_match(entries, "codeberg", Some("foo"), None, 9, false);
        one_match(entries, "codeberg", None, None, 10, false);
        no_matches(entries, "codeberg", Some("bar"), None, false);

        many_matches(entries, "1password", None, None, false);
    }

    #[test]
    fn test_find_by_uuid() {
        let entries = &[
            make_entry("github", Some("foo"), None, &[]),
            make_entry("gitlab", Some("foo"), None, &[]),
            make_entry("gitlab", Some("bar"), None, &[]),
            make_entry(
                "12345678-1234-1234-1234-1234567890ab",
                None,
                None,
                &[],
            ),
            make_entry(
                "12345678-1234-1234-1234-1234567890AC",
                None,
                None,
                &[],
            ),
            make_entry(
                "123456781234123412341234567890AD",
                None,
                None,
                &[],
            ),
        ];

        one_match(entries, &entries[0].0.id, None, None, 0, false);
        one_match(entries, &entries[1].0.id, None, None, 1, false);
        one_match(entries, &entries[2].0.id, None, None, 2, false);
        one_match(
            entries,
            &entries[0].0.id.to_uppercase(),
            None,
            None,
            0,
            false,
        );
        one_match(
            entries,
            &entries[0].0.id.to_lowercase(),
            None,
            None,
            0,
            false,
        );
        one_match(entries, &entries[3].0.id, None, None, 3, false);
        one_match(
            entries,
            "12345678-1234-1234-1234-1234567890ab",
            None,
            None,
            3,
            false,
        );
        no_matches(
            entries,
            "12345678-1234-1234-1234-1234567890AB",
            None,
            None,
            false,
        );
        one_match(
            entries,
            "12345678-1234-1234-1234-1234567890AB",
            None,
            None,
            3,
            true,
        );
        one_match(entries, &entries[4].0.id, None, None, 4, false);
        one_match(
            entries,
            "12345678-1234-1234-1234-1234567890AC",
            None,
            None,
            4,
            false,
        );
        one_match(entries, &entries[5].0.id, None, None, 5, false);
        one_match(
            entries,
            "123456781234123412341234567890AD",
            None,
            None,
            5,
            false,
        );
    }

    #[test]
    fn test_find_by_url_default() {
        let entries = &[
            make_entry("one", None, None, &[("https://one.com/", None)]),
            make_entry(
                "two",
                None,
                None,
                &[("https://two.com/login", None)],
            ),
            make_entry(
                "three",
                None,
                None,
                &[("https://login.three.com/", None)],
            ),
            make_entry("four", None, None, &[("four.com", None)]),
            make_entry(
                "five",
                None,
                None,
                &[("https://five.com:8080/", None)],
            ),
            make_entry("six", None, None, &[("six.com:8080", None)]),
            make_entry(
                "seven",
                None,
                None,
                &[("192.168.0.128:8080", None)],
            ),
        ];

        one_match(entries, "https://one.com/", None, None, 0, false);
        one_match(
            entries,
            "https://login.one.com/",
            None,
            None,
            0,
            false,
        );
        one_match(
            entries,
            "https://one.com:443/",
            None,
            None,
            0,
            false,
        );
        no_matches(entries, "one.com", None, None, false);
        no_matches(entries, "https", None, None, false);
        no_matches(entries, "com", None, None, false);
        no_matches(entries, "https://com/", None, None, false);

        one_match(entries, "https://two.com/", None, None, 1, false);
        one_match(
            entries,
            "https://two.com/other-page",
            None,
            None,
            1,
            false,
        );

        one_match(
            entries,
            "https://login.three.com/",
            None,
            None,
            2,
            false,
        );
        no_matches(entries, "https://three.com/", None, None, false);

        one_match(entries, "https://four.com/", None, None, 3, false);

        one_match(
            entries,
            "https://five.com:8080/",
            None,
            None,
            4,
            false,
        );
        no_matches(entries, "https://five.com/", None, None, false);

        one_match(
            entries,
            "https://six.com:8080/",
            None,
            None,
            5,
            false,
        );
        no_matches(entries, "https://six.com/", None, None, false);

        one_match(
            entries,
            "https://192.168.0.128:8080/",
            None,
            None,
            6,
            false,
        );
    }

    #[test]
    fn test_find_by_url_host() {
        let entries = &[
            make_entry(
                "one",
                None,
                None,
                &[("https://one.com/", Some(crate::api::UriMatchType::Host))],
            ),
            make_entry(
                "two",
                None,
                None,
                &[(
                    "https://login.two.com/start",
                    Some(crate::api::UriMatchType::Host),
                )],
            ),
            make_entry(
                "three",
                None,
                None,
                &[(
                    "https://login.three.com/start",
                    Some(crate::api::UriMatchType::Host),
                )],
            ),
            make_entry(
                "four",
                None,
                None,
                &[("four.com", Some(crate::api::UriMatchType::Host))],
            ),
            make_entry(
                "five",
                None,
                None,
                &[(
                    "https://five.com:8080/",
                    Some(crate::api::UriMatchType::Host),
                )],
            ),
            make_entry(
                "six",
                None,
                None,
                &[("six.com:8080", Some(crate::api::UriMatchType::Host))],
            ),
            make_entry(
                "seven",
                None,
                None,
                &[(
                    "192.168.0.128:8080",
                    Some(crate::api::UriMatchType::Host),
                )],
            ),
        ];

        one_match(entries, "https://one.com/login", None, None, 0, false);
        one_match(
            entries,
            "https://login.two.com/start",
            None,
            None,
            1,
            false,
        );
        // Host matching: three.com won't match login.three.com
        no_matches(
            entries,
            "https://three.com/start",
            None,
            None,
            false,
        );
        one_match(entries, "https://four.com/", None, None, 3, false);
        one_match(
            entries,
            "https://five.com:8080/login",
            None,
            None,
            4,
            false,
        );
        one_match(
            entries,
            "https://six.com:8080/login",
            None,
            None,
            5,
            false,
        );
        one_match(
            entries,
            "https://192.168.0.128:8080/some/path",
            None,
            None,
            6,
            false,
        );
    }

    #[test]
    fn test_find_by_url_starts_with() {
        let entries = &[
            make_entry(
                "one",
                None,
                None,
                &[(
                    "https://one.com/start",
                    Some(crate::api::UriMatchType::StartsWith),
                )],
            ),
            make_entry(
                "two",
                None,
                None,
                &[(
                    "https://login.two.com/start",
                    Some(crate::api::UriMatchType::StartsWith),
                )],
            ),
            make_entry(
                "three",
                None,
                None,
                &[(
                    "https://login.three.com/start",
                    Some(crate::api::UriMatchType::StartsWith),
                )],
            ),
        ];

        one_match(
            entries,
            "https://one.com/starting",
            None,
            None,
            0,
            false,
        );
        one_match(
            entries,
            "https://login.two.com/start",
            None,
            None,
            1,
            false,
        );
        no_matches(
            entries,
            "https://three.com/start",
            None,
            None,
            false,
        );
    }

    #[test]
    fn test_find_by_url_exact() {
        let entries = &[
            make_entry(
                "one",
                None,
                None,
                &[(
                    "https://one.com/",
                    Some(crate::api::UriMatchType::Exact),
                )],
            ),
            make_entry(
                "two",
                None,
                None,
                &[(
                    "https://login.two.com/start",
                    Some(crate::api::UriMatchType::Exact),
                )],
            ),
            make_entry(
                "three",
                None,
                None,
                &[(
                    "https://login.three.com/start",
                    Some(crate::api::UriMatchType::Exact),
                )],
            ),
            make_entry(
                "four",
                None,
                None,
                &[(
                    "https://four.com",
                    Some(crate::api::UriMatchType::Exact),
                )],
            ),
        ];

        one_match(entries, "https://one.com/", None, None, 0, false);
        one_match(
            entries,
            "https://login.two.com/start",
            None,
            None,
            1,
            false,
        );
        no_matches(
            entries,
            "https://three.com/start",
            None,
            None,
            false,
        );
        // Default port normalization
        one_match(
            entries,
            "https://four.com:443/",
            None,
            None,
            3,
            false,
        );
    }

    #[test]
    fn test_find_by_url_regex() {
        let entries = &[
            make_entry(
                "one",
                None,
                None,
                &[(
                    "^https://one\\.com",
                    Some(crate::api::UriMatchType::RegularExpression),
                )],
            ),
            make_entry(
                "two",
                None,
                None,
                &[(
                    "^https://two\\.com/(login|start)",
                    Some(crate::api::UriMatchType::RegularExpression),
                )],
            ),
            make_entry(
                "three",
                None,
                None,
                &[(
                    "^https://(login\\.)?three\\.com/start",
                    Some(crate::api::UriMatchType::RegularExpression),
                )],
            ),
        ];

        one_match(entries, "https://one.com/", None, None, 0, false);
        one_match(
            entries,
            "https://two.com/start",
            None,
            None,
            1,
            false,
        );
        one_match(
            entries,
            "https://three.com/start",
            None,
            None,
            2,
            false,
        );
    }

    #[test]
    fn test_find_by_url_never() {
        let entries = &[
            make_entry(
                "one",
                None,
                None,
                &[(
                    "https://one.com/",
                    Some(crate::api::UriMatchType::Never),
                )],
            ),
            make_entry(
                "two",
                None,
                None,
                &[(
                    "https://login.two.com/start",
                    Some(crate::api::UriMatchType::Never),
                )],
            ),
            make_entry(
                "three",
                None,
                None,
                &[(
                    "https://login.three.com/start",
                    Some(crate::api::UriMatchType::Never),
                )],
            ),
            make_entry(
                "four",
                None,
                None,
                &[(
                    "four.com",
                    Some(crate::api::UriMatchType::Never),
                )],
            ),
            make_entry(
                "five",
                None,
                None,
                &[(
                    "https://five.com:8080/",
                    Some(crate::api::UriMatchType::Never),
                )],
            ),
            make_entry(
                "six",
                None,
                None,
                &[(
                    "six.com:8080",
                    Some(crate::api::UriMatchType::Never),
                )],
            ),
        ];

        no_matches(
            entries,
            "https://one.com/login",
            None,
            None,
            false,
        );
        no_matches(
            entries,
            "https://login.two.com/start",
            None,
            None,
            false,
        );
        no_matches(
            entries,
            "https://three.com/start",
            None,
            None,
            false,
        );
        no_matches(entries, "https://four.com/", None, None, false);
        no_matches(
            entries,
            "https://five.com:8080/login",
            None,
            None,
            false,
        );
        no_matches(
            entries,
            "https://six.com:8080/login",
            None,
            None,
            false,
        );
    }

    #[test]
    fn test_find_with_multiple_urls() {
        let entries = &[
            make_entry(
                "one",
                None,
                None,
                &[
                    (
                        "https://one.com/",
                        Some(crate::api::UriMatchType::Domain),
                    ),
                    (
                        "https://www.one.com/",
                        Some(crate::api::UriMatchType::Domain),
                    ),
                ],
            ),
            make_entry(
                "two",
                None,
                None,
                &[
                    (
                        "https://login.two.com/start",
                        Some(crate::api::UriMatchType::Domain),
                    ),
                    (
                        "https://www.two.com/",
                        Some(crate::api::UriMatchType::Domain),
                    ),
                ],
            ),
        ];

        one_match(
            entries,
            "https://one.com/login",
            None,
            None,
            0,
            false,
        );
        one_match(
            entries,
            "https://www.one.com/login",
            None,
            None,
            0,
            false,
        );
        no_matches(
            entries,
            "https://three.com/login",
            None,
            None,
            false,
        );
    }

}
