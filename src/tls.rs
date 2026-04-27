// src/tls.rs
// TLS 1.3 client (pure software implementation based on RFC 8446)
// Supported cipher suite: TLS_AES_128_GCM_SHA256 (0x1301)
// Supported key exchange: secp256r1 (0x0017)

use alloc::vec::Vec;
use crate::crypto::{sha256, aes, p256, random};

// ========================================================================
// TLS 1.3 constants
// ========================================================================
const TLS_AES_128_GCM_SHA256: u16 = 0x1301;
const GROUP_SECP256R1: u16 = 0x0017;
const TLS_12: u16 = 0x0303; // Record-layer compatibility version
#[allow(dead_code)]
const TLS_13: u16 = 0x0304; // Used in the supported_versions extension

// Content Types
const CT_CHANGE_CIPHER_SPEC: u8 = 20;
const CT_ALERT: u8 = 21;
const CT_HANDSHAKE: u8 = 22;
const CT_APPLICATION_DATA: u8 = 23;
const TLS13_MAX_PLAINTEXT: usize = 16 * 1024;

// Handshake Types
const HT_CLIENT_HELLO: u8 = 1;
const HT_SERVER_HELLO: u8 = 2;
const HT_ENCRYPTED_EXTENSIONS: u8 = 8;
const HT_CERTIFICATE: u8 = 11;
const HT_CERTIFICATE_VERIFY: u8 = 15;
const HT_FINISHED: u8 = 20;

// Extension Types
const EXT_SERVER_NAME: u16 = 0x0000;
const EXT_SUPPORTED_GROUPS: u16 = 0x000a;
const EXT_SIGNATURE_ALGORITHMS: u16 = 0x000d;
const EXT_SUPPORTED_VERSIONS: u16 = 0x002b;
const EXT_KEY_SHARE: u16 = 0x0033;

/// TLS handshake state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TlsState {
    Init,
    WaitServerHello,
    WaitEncryptedExtensions,
    WaitCertOrFinished,
    WaitCertVerify,
    WaitFinished,
    SendClientFinished,
    Ready,
    Error,
}

/// TLS key material.
struct TlsKeys {
    client_write_key: [u8; 16],
    client_write_iv: [u8; 12],
    server_write_key: [u8; 16],
    server_write_iv: [u8; 12],
    client_seq: u64,
    server_seq: u64,
}

/// TLS 1.3 client.
pub struct TlsClient {
    pub state: TlsState,
    // Key exchange
    private_key: [u8; 32],
    public_key: [u8; 65],
    // Transcript hash over all handshake messages
    transcript: sha256::Sha256,
    // Handshake traffic keys
    hs_keys: Option<TlsKeys>,
    // Application traffic keys
    app_keys: Option<TlsKeys>,
    // Receive buffer used to accumulate TCP payloads
    recv_buf: Vec<u8>,
    // Outbound buffer containing TLS records to send
    pub send_buf: Vec<u8>,
    // Decrypted application data
    pub app_data_in: Vec<u8>,
    // Fragment reassembly for plaintext/encrypted handshake messages
    handshake_plain_in: Vec<u8>,
    handshake_encrypted_in: Vec<u8>,
    // Handshake secrets used during Finished verification
    handshake_secret: [u8; 32],
    server_hs_traffic_secret: [u8; 32],
    client_hs_traffic_secret: [u8; 32],
    // Server name (SNI)
    server_name: Vec<u8>,
}

impl TlsClient {
    pub fn new(server_name: &str) -> Self {
        // Generate the P256 keypair in start_handshake() to keep stack usage lower here.
        TlsClient {
            state: TlsState::Init,
            private_key: [0u8; 32],
            public_key: [0u8; 65],
            transcript: sha256::Sha256::new(),
            hs_keys: None,
            app_keys: None,
            recv_buf: Vec::new(),
            send_buf: Vec::new(),
            app_data_in: Vec::new(),
            handshake_plain_in: Vec::new(),
            handshake_encrypted_in: Vec::new(),
            handshake_secret: [0u8; 32],
            server_hs_traffic_secret: [0u8; 32],
            client_hs_traffic_secret: [0u8; 32],
            server_name: Vec::from(server_name.as_bytes()),
        }
    }

    /// Build and queue a ClientHello.
    pub fn start_handshake(&mut self) {
        crate::println!("[TLS] Generating P256 keypair...");
        self.private_key = random::random_bytes_32();
        // Clamp the private key so it stays below the curve order.
        self.private_key[0] &= 0x7F;
        if self.private_key == [0u8; 32] { self.private_key[31] = 1; }

        self.public_key = p256::ecdh_public_key(&self.private_key);
        crate::println!("[TLS] P256 keypair ready. Sending ClientHello...");
        let client_hello = self.build_client_hello();
        // Add ClientHello to the transcript.
        self.transcript.update(&client_hello);
        // Wrap the handshake in a TLS record.
        self.wrap_record(CT_HANDSHAKE, &client_hello);
        self.state = TlsState::WaitServerHello;
    }

    /// Feed inbound TCP data into the TLS parser.
    pub fn feed_data(&mut self, data: &[u8]) {
        self.recv_buf.extend_from_slice(data);
        self.process_records();
    }

    /// Queue application data after the handshake completes.
    pub fn send_app_data(&mut self, data: &[u8]) {
        if self.state != TlsState::Ready { return; }
        if let Some(ref mut keys) = self.app_keys {
            let max_chunk = TLS13_MAX_PLAINTEXT.saturating_sub(1);
            for chunk in data.chunks(max_chunk.max(1)) {
                let mut inner = Vec::with_capacity(chunk.len() + 1);
                inner.extend_from_slice(chunk);
                inner.push(CT_APPLICATION_DATA);
                let encrypted = encrypt_record(keys, CT_APPLICATION_DATA, &inner);
                self.send_buf.extend_from_slice(&encrypted);
            }
        }
    }

    /// Return true when there is pending outbound data.
    pub fn has_data_to_send(&self) -> bool { !self.send_buf.is_empty() }

    /// Borrow the pending outbound TLS bytes.
    pub fn send_buf(&self) -> &[u8] {
        &self.send_buf
    }

    /// Drop bytes that were successfully written to the TCP socket.
    pub fn consume_send_buf(&mut self, count: usize) {
        let drain_len = core::cmp::min(count, self.send_buf.len());
        self.send_buf.drain(..drain_len);
    }

    // ====================================================================
    // Internal implementation details
    // ====================================================================

    fn build_client_hello(&self) -> Vec<u8> {
        let mut msg = Vec::new();

        // Handshake header (type + length is filled in later)
        let mut body = Vec::new();

        // Legacy version: TLS 1.2
        body.extend_from_slice(&TLS_12.to_be_bytes());

        // Random (32 bytes)
        let client_random = random::random_bytes_32();
        body.extend_from_slice(&client_random);

        // Legacy Session ID (empty)
        body.push(0); // length = 0

        // Cipher Suites
        body.extend_from_slice(&2u16.to_be_bytes()); // length
        body.extend_from_slice(&TLS_AES_128_GCM_SHA256.to_be_bytes());

        // Compression Methods (null only)
        body.push(1); // length
        body.push(0); // null compression

        // Extensions
        let extensions = self.build_extensions();
        body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        body.extend_from_slice(&extensions);

        // Handshake header: type(1) + length(3)
        msg.push(HT_CLIENT_HELLO);
        let len = body.len() as u32;
        msg.push((len >> 16) as u8);
        msg.push((len >> 8) as u8);
        msg.push(len as u8);
        msg.extend_from_slice(&body);

        msg
    }

    fn build_extensions(&self) -> Vec<u8> {
        let mut ext = Vec::new();

        // SNI (Server Name Indication)
        if !self.server_name.is_empty() {
            let mut sni_data = Vec::new();
            // ServerNameList length
            let entry_len = self.server_name.len() as u16 + 3;
            sni_data.extend_from_slice(&entry_len.to_be_bytes());
            sni_data.push(0); // host_name type
            sni_data.extend_from_slice(&(self.server_name.len() as u16).to_be_bytes());
            sni_data.extend_from_slice(&self.server_name);
            push_extension(&mut ext, EXT_SERVER_NAME, &sni_data);
        }

        // Supported Groups: secp256r1
        let groups = [
            0x00, 0x02, // list length
            0x00, 0x17, // secp256r1
        ];
        push_extension(&mut ext, EXT_SUPPORTED_GROUPS, &groups);

        // Signature Algorithms
        let sig_algs = [
            0x00, 0x04, // list length
            0x04, 0x03, // ecdsa_secp256r1_sha256
            0x08, 0x04, // rsa_pss_rsae_sha256
        ];
        push_extension(&mut ext, EXT_SIGNATURE_ALGORITHMS, &sig_algs);

        // Supported Versions: TLS 1.3
        let versions = [
            0x02, // vector length in bytes: one ProtocolVersion entry
            0x03, 0x04, // TLS 1.3
        ];
        push_extension(&mut ext, EXT_SUPPORTED_VERSIONS, &versions);

        // Key Share: secp256r1 public key
        let mut ks_data = Vec::new();
        let entry_len = 2 + 2 + self.public_key.len(); // group(2) + key_len(2) + key
        ks_data.extend_from_slice(&(entry_len as u16).to_be_bytes()); // client_shares length
        ks_data.extend_from_slice(&GROUP_SECP256R1.to_be_bytes());
        ks_data.extend_from_slice(&(self.public_key.len() as u16).to_be_bytes());
        ks_data.extend_from_slice(&self.public_key);
        push_extension(&mut ext, EXT_KEY_SHARE, &ks_data);

        ext
    }

    fn wrap_record(&mut self, content_type: u8, data: &[u8]) {
        // TLS record = ContentType(1) + Version(2) + Length(2) + Data
        self.send_buf.push(content_type);
        self.send_buf.extend_from_slice(&TLS_12.to_be_bytes());
        self.send_buf.extend_from_slice(&(data.len() as u16).to_be_bytes());
        self.send_buf.extend_from_slice(data);
    }

    fn process_records(&mut self) {
        loop {
            if self.recv_buf.len() < 5 { return; }

            let content_type = self.recv_buf[0];
            let length = u16::from_be_bytes([self.recv_buf[3], self.recv_buf[4]]) as usize;

            if self.recv_buf.len() < 5 + length { return; }

            let record_data = self.recv_buf[5..5 + length].to_vec();
            self.recv_buf.drain(..5 + length);

            match content_type {
                CT_HANDSHAKE => self.handle_handshake(&record_data),
                CT_CHANGE_CIPHER_SPEC => { /* TLS 1.3: ignore for compatibility */ }
                CT_APPLICATION_DATA => self.handle_encrypted_record(&record_data),
                CT_ALERT => {
                    if record_data.len() >= 2 {
                        crate::println!(
                            "[TLS] Alert received: level={} description={}",
                            record_data[0],
                            record_data[1]
                        );
                    } else {
                        crate::println!("[TLS] Short alert record received");
                    }
                    self.state = TlsState::Error;
                    return;
                }
                other => {
                    crate::println!("[TLS] Unexpected record content type: {}", other);
                }
            }

            if self.state == TlsState::Error { return; }
        }
    }

    fn handle_handshake(&mut self, data: &[u8]) {
        self.handshake_plain_in.extend_from_slice(data);

        loop {
            if self.handshake_plain_in.len() < 4 {
                return;
            }

            let hs_type = self.handshake_plain_in[0];
            let hs_len = ((self.handshake_plain_in[1] as usize) << 16)
                | ((self.handshake_plain_in[2] as usize) << 8)
                | (self.handshake_plain_in[3] as usize);
            let full_len = 4 + hs_len;

            if self.handshake_plain_in.len() < full_len {
                return;
            }

            let message = self.handshake_plain_in[..full_len].to_vec();
            self.handshake_plain_in.drain(..full_len);
            let hs_body = &message[4..];

            crate::println!(
                "[TLS] Plain handshake message type={} len={}",
                hs_type,
                hs_len
            );

            match (self.state, hs_type) {
                (TlsState::WaitServerHello, HT_SERVER_HELLO) => {
                    self.transcript.update(&message);
                    self.handle_server_hello(hs_body);
                }
                _ => {}
            }

            if self.state == TlsState::Error {
                return;
            }
        }
    }

    fn handle_server_hello(&mut self, body: &[u8]) {
        if body.len() < 34 {
            crate::println!("[TLS] ServerHello too short");
            self.state = TlsState::Error;
            return;
        }

        let mut offset = 0;
        // server version (2) + random (32)
        offset += 34;

        // session_id
        if offset >= body.len() {
            crate::println!("[TLS] ServerHello missing session id");
            self.state = TlsState::Error;
            return;
        }
        let sid_len = body[offset] as usize;
        offset += 1 + sid_len;

        // cipher_suite (2)
        if offset + 2 > body.len() {
            crate::println!("[TLS] ServerHello missing cipher suite");
            self.state = TlsState::Error;
            return;
        }
        let cipher = u16::from_be_bytes([body[offset], body[offset + 1]]);
        if cipher != TLS_AES_128_GCM_SHA256 {
            crate::println!("[TLS] Unsupported cipher suite: {:#06x}", cipher);
            self.state = TlsState::Error;
            return;
        }
        offset += 2;

        // compression_method (1)
        offset += 1;

        // extensions
        if offset + 2 > body.len() {
            crate::println!("[TLS] ServerHello missing extensions");
            self.state = TlsState::Error;
            return;
        }
        let ext_len = u16::from_be_bytes([body[offset], body[offset + 1]]) as usize;
        offset += 2;

        let ext_end = offset + ext_len;
        let mut server_public_key: Option<Vec<u8>> = None;

        while offset + 4 <= ext_end && offset + 4 <= body.len() {
            let ext_type = u16::from_be_bytes([body[offset], body[offset + 1]]);
            let ext_data_len = u16::from_be_bytes([body[offset + 2], body[offset + 3]]) as usize;
            offset += 4;

            if offset + ext_data_len > body.len() { break; }

            if ext_type == EXT_KEY_SHARE {
                // group(2) + key_length(2) + key
                if ext_data_len >= 4 {
                    let key_len = u16::from_be_bytes([body[offset + 2], body[offset + 3]]) as usize;
                    if ext_data_len >= 4 + key_len {
                        server_public_key = Some(body[offset + 4..offset + 4 + key_len].to_vec());
                    }
                }
            }
            offset += ext_data_len;
        }

        // Compute the ECDH shared secret.
        let server_pk = match server_public_key {
            Some(pk) => pk,
            None => {
                crate::println!("[TLS] ServerHello missing P-256 key share");
                self.state = TlsState::Error;
                return;
            }
        };

        let shared_secret = match p256::ecdh_shared_secret(&self.private_key, &server_pk) {
            Some(s) => s,
            None => {
                crate::println!("[TLS] P-256 shared secret derivation failed");
                self.state = TlsState::Error;
                return;
            }
        };

        // TLS 1.3 key schedule
        self.derive_handshake_keys(&shared_secret);
        self.state = TlsState::WaitEncryptedExtensions;
    }

    fn derive_handshake_keys(&mut self, shared_secret: &[u8; 32]) {
        let zeros = [0u8; 32];

        // Early Secret
        let early_secret = sha256::hkdf_extract(&zeros, &zeros);

        // Derive-Secret(early_secret, "derived", "")
        let derived1 = derive_secret(&early_secret, b"derived", &sha256::sha256(&[]));

        // Handshake Secret
        let handshake_secret = sha256::hkdf_extract(&derived1, shared_secret);
        self.handshake_secret = handshake_secret;

        // Current transcript hash (ClientHello + ServerHello)
        let transcript_hash = self.transcript_hash();

        // client/server handshake traffic secret
        self.client_hs_traffic_secret = derive_secret(&handshake_secret, b"c hs traffic", &transcript_hash);
        self.server_hs_traffic_secret = derive_secret(&handshake_secret, b"s hs traffic", &transcript_hash);

        // Derive handshake traffic keys.
        let s_key = hkdf_expand_label(&self.server_hs_traffic_secret, b"key", &[], 16);
        let s_iv = hkdf_expand_label(&self.server_hs_traffic_secret, b"iv", &[], 12);
        let c_key = hkdf_expand_label(&self.client_hs_traffic_secret, b"key", &[], 16);
        let c_iv = hkdf_expand_label(&self.client_hs_traffic_secret, b"iv", &[], 12);

        let mut sk = [0u8; 16]; sk.copy_from_slice(&s_key);
        let mut si = [0u8; 12]; si.copy_from_slice(&s_iv);
        let mut ck = [0u8; 16]; ck.copy_from_slice(&c_key);
        let mut ci = [0u8; 12]; ci.copy_from_slice(&c_iv);

        self.hs_keys = Some(TlsKeys {
            server_write_key: sk, server_write_iv: si,
            client_write_key: ck, client_write_iv: ci,
            server_seq: 0, client_seq: 0,
        });
    }

    fn handle_encrypted_record(&mut self, record_data: &[u8]) {
        // Choose handshake keys or application keys based on the current state.
        let keys_ref = if self.state == TlsState::Ready {
            &mut self.app_keys
        } else {
            &mut self.hs_keys
        };

        let keys = match keys_ref {
            Some(k) => k,
            None => { self.state = TlsState::Error; return; }
        };

        // AES-GCM decryption
        if record_data.len() < 17 { return; } // Minimum: 1 content_type byte + 16-byte tag
        let ciphertext = &record_data[..record_data.len() - 16];
        let tag_slice = &record_data[record_data.len() - 16..];
        let mut tag = [0u8; 16];
        tag.copy_from_slice(tag_slice);

        // Nonce = IV XOR sequence_number
        let mut nonce = keys.server_write_iv;
        let seq_bytes = keys.server_seq.to_be_bytes();
        for i in 0..8 { nonce[4 + i] ^= seq_bytes[i]; }
        keys.server_seq += 1;

        // AAD = record header (5 bytes)
        let aad = [CT_APPLICATION_DATA, 0x03, 0x03,
                    ((record_data.len() >> 8) & 0xFF) as u8,
                    (record_data.len() & 0xFF) as u8];

        let plaintext = match aes::aes_gcm_decrypt(
            &keys.server_write_key, &nonce, &aad, ciphertext, &tag
        ) {
            Some(pt) => pt,
            None => { self.state = TlsState::Error; return; }
        };

        if plaintext.is_empty() { return; }

        // TLS 1.3 inner plaintext is content || type || zero-padding.
        let mut content_type_index = None;
        for (index, byte) in plaintext.iter().enumerate().rev() {
            if *byte != 0 {
                content_type_index = Some(index);
                break;
            }
        }

        let content_type_index = match content_type_index {
            Some(index) => index,
            None => {
                crate::println!("[TLS] Empty inner plaintext after removing padding");
                self.state = TlsState::Error;
                return;
            }
        };

        let actual_ct = plaintext[content_type_index];
        let inner_data = &plaintext[..content_type_index];

        match actual_ct {
            CT_HANDSHAKE => self.handle_encrypted_handshake(inner_data),
            CT_APPLICATION_DATA => self.app_data_in.extend_from_slice(inner_data),
            CT_ALERT => { self.state = TlsState::Error; }
            other => {
                crate::println!("[TLS] Unexpected inner content type: {}", other);
            }
        }
    }

    fn handle_encrypted_handshake(&mut self, data: &[u8]) {
        self.handshake_encrypted_in.extend_from_slice(data);

        loop {
            if self.handshake_encrypted_in.len() < 4 {
                return;
            }

            let hs_type = self.handshake_encrypted_in[0];
            let hs_len = ((self.handshake_encrypted_in[1] as usize) << 16)
                | ((self.handshake_encrypted_in[2] as usize) << 8)
                | (self.handshake_encrypted_in[3] as usize);
            let end = 4 + hs_len;
            if self.handshake_encrypted_in.len() < end {
                return;
            }

            let message = self.handshake_encrypted_in[..end].to_vec();
            self.handshake_encrypted_in.drain(..end);

            self.transcript.update(&message);

            crate::println!(
                "[TLS] Encrypted handshake message type={} len={}",
                hs_type,
                hs_len
            );

            match (self.state, hs_type) {
                (TlsState::WaitEncryptedExtensions, HT_ENCRYPTED_EXTENSIONS) => {
                    self.state = TlsState::WaitCertOrFinished;
                }
                (TlsState::WaitCertOrFinished, HT_CERTIFICATE) => {
                    self.state = TlsState::WaitCertVerify;
                }
                (TlsState::WaitCertOrFinished, HT_FINISHED) |
                (TlsState::WaitFinished, HT_FINISHED) => {
                    // Finished verification is still skipped for now.
                    self.complete_handshake();
                    return;
                }
                (TlsState::WaitCertVerify, HT_CERTIFICATE_VERIFY) => {
                    // Certificate verification is still skipped for now.
                    self.state = TlsState::WaitFinished;
                }
                _ => {}
            }

            if self.state == TlsState::Error {
                return;
            }
        }
    }

    fn complete_handshake(&mut self) {
        // Send Client Finished.
        let finished_key = hkdf_expand_label(
            &self.client_hs_traffic_secret, b"finished", &[], 32);
        let transcript_hash = self.transcript_hash();
        let verify_data = sha256::hmac_sha256(&finished_key, &transcript_hash);

        // TLS 1.3 application traffic secrets are derived from the transcript
        // up to Server Finished, before Client Finished is appended.
        let app_transcript_hash = transcript_hash;

        // Build the Finished message.
        let mut finished_msg = Vec::new();
        finished_msg.push(HT_FINISHED);
        finished_msg.push(0); finished_msg.push(0); finished_msg.push(32); // length = 32
        finished_msg.extend_from_slice(&verify_data);

        // Encrypt and queue it.
        if let Some(ref mut keys) = self.hs_keys {
            let mut inner = finished_msg.clone();
            inner.push(CT_HANDSHAKE); // inner content type

            let encrypted = encrypt_record(keys, CT_APPLICATION_DATA, &inner);
            self.send_buf.extend_from_slice(&encrypted);
        }

        // Add Finished to the transcript.
        self.transcript.update(&finished_msg);

        // Derive application traffic keys.
        let derived2 = derive_secret(&self.handshake_secret, b"derived", &sha256::sha256(&[]));
        let master_secret = sha256::hkdf_extract(&derived2, &[0u8; 32]);

        let c_app_secret = derive_secret(&master_secret, b"c ap traffic", &app_transcript_hash);
        let s_app_secret = derive_secret(&master_secret, b"s ap traffic", &app_transcript_hash);

        let sk = hkdf_expand_label(&s_app_secret, b"key", &[], 16);
        let si = hkdf_expand_label(&s_app_secret, b"iv", &[], 12);
        let ck = hkdf_expand_label(&c_app_secret, b"key", &[], 16);
        let ci = hkdf_expand_label(&c_app_secret, b"iv", &[], 12);

        let mut skey = [0u8; 16]; skey.copy_from_slice(&sk);
        let mut siv = [0u8; 12]; siv.copy_from_slice(&si);
        let mut ckey = [0u8; 16]; ckey.copy_from_slice(&ck);
        let mut civ = [0u8; 12]; civ.copy_from_slice(&ci);

        self.app_keys = Some(TlsKeys {
            server_write_key: skey, server_write_iv: siv,
            client_write_key: ckey, client_write_iv: civ,
            server_seq: 0, client_seq: 0,
        });

        self.state = TlsState::Ready;
        crate::println!("[TLS] Handshake complete! TLS 1.3 session established.");
    }

    fn transcript_hash(&self) -> [u8; 32] {
        let hasher = self.transcript.clone();
        hasher.finalize()
    }
}

// ========================================================================
// TLS 1.3 key-schedule helpers
// ========================================================================

/// Derive-Secret(Secret, Label, TranscriptHash)
fn derive_secret(secret: &[u8; 32], label: &[u8], transcript_hash: &[u8; 32]) -> [u8; 32] {
    let expanded = hkdf_expand_label(secret, label, transcript_hash, 32);
    let mut result = [0u8; 32];
    result.copy_from_slice(&expanded);
    result
}

/// HKDF-Expand-Label(Secret, Label, Context, Length)
fn hkdf_expand_label(secret: &[u8; 32], label: &[u8], context: &[u8], length: usize) -> Vec<u8> {
    // HkdfLabel = Length(2) + "tls13 " + Label + Context
    let mut info = Vec::new();
    info.extend_from_slice(&(length as u16).to_be_bytes());

    // Label with the "tls13 " prefix
    let full_label_len = 6 + label.len();
    info.push(full_label_len as u8);
    info.extend_from_slice(b"tls13 ");
    info.extend_from_slice(label);

    // Context
    info.push(context.len() as u8);
    info.extend_from_slice(context);

    sha256::hkdf_expand(secret, &info, length)
}

/// Encrypt a TLS record with AES-GCM.
fn encrypt_record(keys: &mut TlsKeys, content_type: u8, plaintext: &[u8]) -> Vec<u8> {
    // Nonce
    let mut nonce = keys.client_write_iv;
    let seq_bytes = keys.client_seq.to_be_bytes();
    for i in 0..8 { nonce[4 + i] ^= seq_bytes[i]; }
    keys.client_seq += 1;

    let record_len = plaintext.len() + 16; // ciphertext + tag

    // AAD = record header
    let aad = [content_type, 0x03, 0x03,
               ((record_len >> 8) & 0xFF) as u8,
               (record_len & 0xFF) as u8];

    let (ciphertext, tag) = aes::aes_gcm_encrypt(
        &keys.client_write_key, &nonce, &aad, plaintext);

    // TLS record
    let mut record = Vec::new();
    record.push(content_type);
    record.extend_from_slice(&TLS_12.to_be_bytes());
    record.extend_from_slice(&(record_len as u16).to_be_bytes());
    record.extend_from_slice(&ciphertext);
    record.extend_from_slice(&tag);
    record
}

/// Extension builder helper.
fn push_extension(buf: &mut Vec<u8>, ext_type: u16, data: &[u8]) {
    buf.extend_from_slice(&ext_type.to_be_bytes());
    buf.extend_from_slice(&(data.len() as u16).to_be_bytes());
    buf.extend_from_slice(data);
}
