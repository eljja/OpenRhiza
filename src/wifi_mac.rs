// src/wifi_mac.rs
use alloc::vec::Vec;
use alloc::string::String;
use core::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WifiState {
    Idle,
    Scanning,
    Authenticating,
    Associated,
    Error,
}

// Global state for 802.11 MAC
static WIFI_STATE: AtomicU8 = AtomicU8::new(0); // 0=Idle, 1=Scanning, 2=Authenticating, 3=Associated, 4=Error

pub struct WifiMac {
    pub ssid: String,
    pub bssid: [u8; 6],
    pub state: WifiState,
    pub psk: Option<String>,
}

pub struct EapolKeyFrame {
    pub version: u8,
    pub type_: u8,
    pub length: u16,
    pub descriptor_type: u8,
    pub key_info: u16,
    pub key_length: u16,
    pub replay_counter: u64,
    pub key_nonce: [u8; 32],
    pub key_iv: [u8; 16],
    pub key_rsc: u64,
    pub key_id: u64,
    pub key_mic: [u8; 16],
    pub key_data_length: u16,
}

impl WifiMac {
    pub fn new() -> Self {
        WifiMac {
            ssid: String::new(),
            bssid: [0; 6],
            state: WifiState::Idle,
            psk: None,
        }
    }

    pub fn set_state(&mut self, state: WifiState) {
        self.state = state;
        let val = match state {
            WifiState::Idle => 0,
            WifiState::Scanning => 1,
            WifiState::Authenticating => 2,
            WifiState::Associated => 3,
            WifiState::Error => 4,
        };
        WIFI_STATE.store(val, Ordering::Relaxed);
        crate::println!("[Wi-Fi MAC] State changed to {:?}", state);
    }

    pub fn get_global_state() -> WifiState {
        match WIFI_STATE.load(Ordering::Relaxed) {
            1 => WifiState::Scanning,
            2 => WifiState::Authenticating,
            3 => WifiState::Associated,
            4 => WifiState::Error,
            _ => WifiState::Idle,
        }
    }

    /// Initiate a scan for an SSID
    pub fn scan(&mut self, target_ssid: &str) {
        crate::println!("[Wi-Fi MAC] Initiating scan for '{}'", target_ssid);
        self.ssid = String::from(target_ssid);
        self.set_state(WifiState::Scanning);
        
        // TODO: Actually construct 802.11 Probe Request frame and inject into TX ring
        self.simulate_scan_results();
    }

    /// Simulates receiving a Probe Response or Beacon
    fn simulate_scan_results(&mut self) {
        crate::println!("[Wi-Fi MAC] Found network '{}' (BSSID: c0:ff:ee:12:34:56, WPA2-PSK)", self.ssid);
        self.bssid = [0xc0, 0xff, 0xee, 0x12, 0x34, 0x56];
        self.set_state(WifiState::Idle);
    }

    /// Connect sequence: Auth -> Assoc -> 4-way EAPOL Handshake
    pub fn connect(&mut self, psk: &str) {
        if self.bssid == [0; 6] {
            crate::println!("[Wi-Fi MAC] Cannot connect without scanning first.");
            return;
        }

        self.psk = Some(String::from(psk));
        self.set_state(WifiState::Authenticating);

        crate::println!("[Wi-Fi MAC] Sending 802.11 Authentication Frame (Open System)...");
        // TODO: Construct and send 802.11 Auth Frame
        crate::println!("[Wi-Fi MAC] Received 802.11 Authentication Response (Success).");
        
        crate::println!("[Wi-Fi MAC] Sending 802.11 Association Request Frame...");
        // TODO: Construct and send 802.11 Association Request
        crate::println!("[Wi-Fi MAC] Received 802.11 Association Response (Success).");

        // Proceed to 4-way handshake
        crate::println!("[Wi-Fi MAC] Entering WPA2 4-Way Handshake Phase.");
        if self.perform_wpa2_handshake() {
            crate::println!("[Wi-Fi MAC] 4-Way Handshake Successful. Keys Installed.");
            self.set_state(WifiState::Associated);
        } else {
            crate::println!("[Wi-Fi MAC] 4-Way Handshake Failed.");
            self.set_state(WifiState::Error);
        }
    }

    /// WPA2 4-Way Handshake implementation over EAPOL
    fn perform_wpa2_handshake(&self) -> bool {
        // Scaffolding for the 4-way handshake 
        // 1. AP sends Message 1 (ANonce)
        // 2. Client sends Message 2 (SNonce + MIC)
        // 3. AP sends Message 3 (GTK + MIC)
        // 4. Client sends Message 4 (Ack + MIC)
        
        crate::println!("[Wi-Fi MAC EAPOL] Waiting for Msg 1/4 (AP -> Client) [ANonce]...");
        crate::println!("[Wi-Fi MAC EAPOL] Received Msg 1/4. Generating PTK...");
        
        // Pseudo-crypto: PTK = PRF-512(PMK, "Pairwise key expansion", MinMax(Mac), MinMax(Nonce))
        if let Some(psk) = &self.psk {
            let pmk = pbkdf2_sha1_mock(psk, &self.ssid, 4096, 32);
            crate::println!("[Wi-Fi MAC Crypto] Derived PMK (mocked): {:?}", &pmk[0..4]);
        }

        crate::println!("[Wi-Fi MAC EAPOL] Sending Msg 2/4 (Client -> AP) [SNonce + MIC]...");
        
        crate::println!("[Wi-Fi MAC EAPOL] Waiting for Msg 3/4 (AP -> Client) [GTK + MIC]...");
        crate::println!("[Wi-Fi MAC EAPOL] Received Msg 3/4. Verifying MIC and extracting GTK...");
        
        crate::println!("[Wi-Fi MAC EAPOL] Sending Msg 4/4 (Client -> AP) [Ack]...");
        
        // Mock success
        true
    }

    /// Translates standard 802.3 Ethernet frame to 802.11 Data frame with CCMP encryption
    pub fn transmit_802_11_data_frame(&self, ethernet_frame: &[u8]) -> Vec<u8> {
        let mut wifi_frame = Vec::new();

        // 802.11 Frame Control
        let fc: u16 = 0x08; // Frame Type: Data, Subtype: Data
        wifi_frame.push((fc & 0xFF) as u8);
        wifi_frame.push((fc >> 8) as u8);

        // Duration / ID (mocked)
        wifi_frame.push(0);
        wifi_frame.push(0);

        // Address 1 (BSSID / AP MAC)
        wifi_frame.extend_from_slice(&self.bssid);
        // Address 2 (Source MAC - let's assume all zeros for mock)
        wifi_frame.extend_from_slice(&[0; 6]);
        // Address 3 (Destination MAC - extract from ethernet_frame)
        wifi_frame.extend_from_slice(&ethernet_frame[0..6]);
        
        // Seq Control
        wifi_frame.push(0);
        wifi_frame.push(0);

        // CCMP Header (8 bytes)
        wifi_frame.extend_from_slice(&[0; 8]); // Mock PN/KeyID

        // CCMP Encrypted Payload (mocked as plaintext for now)
        wifi_frame.extend_from_slice(&ethernet_frame[14..]);

        // MIC (8 bytes)
        wifi_frame.extend_from_slice(&[0; 8]);
        
        // FCS (4 bytes)
        wifi_frame.extend_from_slice(&[0; 4]);

        wifi_frame
    }
}

/// Mocks the PBKDF2 calculation for WPA2 PMK derivation
fn pbkdf2_sha1_mock(passphrase: &str, ssid: &str, _iterations: u32, out_len: usize) -> Vec<u8> {
    // In real implementation, this requires HMAC-SHA1
    let mut out = Vec::with_capacity(out_len);
    let mut hash = 0u8;
    for c in passphrase.bytes().chain(ssid.bytes()) {
        hash = hash.wrapping_add(c);
    }
    for i in 0..out_len {
        out.push(hash.wrapping_add(i as u8));
    }
    out
}
