use alloc::string::String;

const CHOSEONG_COMPAT: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ',
    'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

#[derive(Clone, Copy)]
enum HangulKey {
    Consonant { choseong: u8, jongseong: u8 },
    Vowel(u8),
}

#[derive(Default)]
pub struct HangulIme {
    enabled: bool,
    choseong: Option<u8>,
    jungseong: Option<u8>,
    jongseong: Option<u8>,
}

pub struct HangulStep {
    pub commit: String,
    pub preview: Option<char>,
}

impl HangulStep {
    fn empty(preview: Option<char>) -> Self {
        Self {
            commit: String::new(),
            preview,
        }
    }
}

impl HangulIme {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            choseong: None,
            jungseong: None,
            jongseong: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    pub fn take_commit_before_toggle(&mut self) -> String {
        self.commit_composition()
    }

    pub fn preview_char(&self) -> Option<char> {
        self.current_preview_char()
    }

    pub fn has_composition(&self) -> bool {
        self.choseong.is_some() || self.jungseong.is_some() || self.jongseong.is_some()
    }

    pub fn commit_pending(&mut self) -> String {
        self.commit_composition()
    }

    pub fn backspace(&mut self) -> bool {
        if let Some(jong) = self.jongseong {
            if let Some((left, _right)) = split_compound_jong(jong) {
                self.jongseong = Some(left);
            } else {
                self.jongseong = None;
            }
            return true;
        }
        if let Some(jung) = self.jungseong {
            if let Some(base) = split_compound_vowel(jung) {
                self.jungseong = Some(base);
            } else {
                self.jungseong = None;
                if self.choseong == Some(11) {
                    self.choseong = None;
                }
            }
            return true;
        }
        if self.choseong.is_some() {
            self.choseong = None;
            return true;
        }
        false
    }

    pub fn process_ascii(&mut self, ch: char) -> HangulStep {
        let Some(key) = map_ascii_to_hangul_key(ch) else {
            let mut commit = self.commit_composition();
            commit.push(ch);
            return HangulStep {
                commit,
                preview: None,
            };
        };

        match key {
            HangulKey::Consonant { choseong, jongseong } => self.process_consonant(choseong, jongseong),
            HangulKey::Vowel(vowel) => self.process_vowel(vowel),
        }
    }

    fn process_consonant(&mut self, choseong: u8, jongseong: u8) -> HangulStep {
        if self.choseong.is_none() {
            self.choseong = Some(choseong);
            return HangulStep::empty(self.current_preview_char());
        }

        if self.jungseong.is_none() {
            let commit = self.commit_composition();
            self.choseong = Some(choseong);
            return HangulStep {
                commit,
                preview: self.current_preview_char(),
            };
        }

        if self.jongseong.is_none() {
            if jongseong != 0 {
                self.jongseong = Some(jongseong);
                return HangulStep::empty(self.current_preview_char());
            }

            let commit = self.commit_composition();
            self.choseong = Some(choseong);
            return HangulStep {
                commit,
                preview: self.current_preview_char(),
            };
        }

        if let Some(combined) = combine_jong(self.jongseong.unwrap(), jongseong) {
            self.jongseong = Some(combined);
            return HangulStep::empty(self.current_preview_char());
        }

        let commit = self.commit_composition();
        self.choseong = Some(choseong);
        self.jungseong = None;
        self.jongseong = None;
        HangulStep {
            commit,
            preview: self.current_preview_char(),
        }
    }

    fn process_vowel(&mut self, vowel: u8) -> HangulStep {
        if self.choseong.is_none() {
            self.choseong = Some(11);
            self.jungseong = Some(vowel);
            return HangulStep::empty(self.current_preview_char());
        }

        if self.jungseong.is_none() {
            self.jungseong = Some(vowel);
            return HangulStep::empty(self.current_preview_char());
        }

        if self.jongseong.is_none() {
            if let Some(combined) = combine_vowel(self.jungseong.unwrap(), vowel) {
                self.jungseong = Some(combined);
                return HangulStep::empty(self.current_preview_char());
            }

            let commit = self.commit_composition();
            self.choseong = Some(11);
            self.jungseong = Some(vowel);
            return HangulStep {
                commit,
                preview: self.current_preview_char(),
            };
        }

        let current_jong = self.jongseong.unwrap();
        let (stay_jong, next_choseong) = if let Some((left, right)) = split_compound_jong(current_jong) {
            (Some(left), jong_to_choseong(right))
        } else {
            (None, jong_to_choseong(current_jong))
        };

        let committed = compose_syllable(
            self.choseong.unwrap(),
            self.jungseong.unwrap(),
            stay_jong.unwrap_or(0),
        );
        let mut commit = String::new();
        commit.push(committed);

        self.choseong = Some(next_choseong.unwrap_or(11));
        self.jungseong = Some(vowel);
        self.jongseong = None;
        HangulStep {
            commit,
            preview: self.current_preview_char(),
        }
    }

    fn commit_composition(&mut self) -> String {
        let Some(ch) = self.current_preview_char() else {
            return String::new();
        };
        self.choseong = None;
        self.jungseong = None;
        self.jongseong = None;
        let mut out = String::new();
        out.push(ch);
        out
    }

    fn current_preview_char(&self) -> Option<char> {
        let choseong = self.choseong?;
        let Some(jungseong) = self.jungseong else {
            return CHOSEONG_COMPAT.get(choseong as usize).copied();
        };
        Some(compose_syllable(
            choseong,
            jungseong,
            self.jongseong.unwrap_or(0),
        ))
    }
}

fn compose_syllable(choseong: u8, jungseong: u8, jongseong: u8) -> char {
    let code = 0xAC00u32
        + (choseong as u32 * 21 * 28)
        + (jungseong as u32 * 28)
        + jongseong as u32;
    char::from_u32(code).unwrap_or('?')
}

fn map_ascii_to_hangul_key(ch: char) -> Option<HangulKey> {
    match ch {
        'r' => Some(HangulKey::Consonant { choseong: 0, jongseong: 1 }),
        'R' => Some(HangulKey::Consonant { choseong: 1, jongseong: 2 }),
        's' => Some(HangulKey::Consonant { choseong: 2, jongseong: 4 }),
        'e' => Some(HangulKey::Consonant { choseong: 3, jongseong: 7 }),
        'E' => Some(HangulKey::Consonant { choseong: 4, jongseong: 0 }),
        'f' => Some(HangulKey::Consonant { choseong: 5, jongseong: 8 }),
        'a' => Some(HangulKey::Consonant { choseong: 6, jongseong: 16 }),
        'q' => Some(HangulKey::Consonant { choseong: 7, jongseong: 17 }),
        'Q' => Some(HangulKey::Consonant { choseong: 8, jongseong: 0 }),
        't' => Some(HangulKey::Consonant { choseong: 9, jongseong: 19 }),
        'T' => Some(HangulKey::Consonant { choseong: 10, jongseong: 20 }),
        'd' => Some(HangulKey::Consonant { choseong: 11, jongseong: 21 }),
        'w' => Some(HangulKey::Consonant { choseong: 12, jongseong: 22 }),
        'W' => Some(HangulKey::Consonant { choseong: 13, jongseong: 0 }),
        'c' => Some(HangulKey::Consonant { choseong: 14, jongseong: 23 }),
        'z' => Some(HangulKey::Consonant { choseong: 15, jongseong: 24 }),
        'x' => Some(HangulKey::Consonant { choseong: 16, jongseong: 25 }),
        'v' => Some(HangulKey::Consonant { choseong: 17, jongseong: 26 }),
        'g' => Some(HangulKey::Consonant { choseong: 18, jongseong: 27 }),
        'k' => Some(HangulKey::Vowel(0)),
        'o' => Some(HangulKey::Vowel(1)),
        'i' => Some(HangulKey::Vowel(2)),
        'O' => Some(HangulKey::Vowel(3)),
        'j' => Some(HangulKey::Vowel(4)),
        'p' => Some(HangulKey::Vowel(5)),
        'u' => Some(HangulKey::Vowel(6)),
        'P' => Some(HangulKey::Vowel(7)),
        'h' => Some(HangulKey::Vowel(8)),
        'y' => Some(HangulKey::Vowel(12)),
        'n' => Some(HangulKey::Vowel(13)),
        'b' => Some(HangulKey::Vowel(17)),
        'm' => Some(HangulKey::Vowel(18)),
        'l' => Some(HangulKey::Vowel(20)),
        _ => None,
    }
}

fn combine_vowel(first: u8, second: u8) -> Option<u8> {
    match (first, second) {
        (8, 0) => Some(9),   // ㅗ + ㅏ = ㅘ
        (8, 1) => Some(10),  // ㅗ + ㅐ = ㅙ
        (8, 20) => Some(11), // ㅗ + ㅣ = ㅚ
        (13, 4) => Some(14), // ㅜ + ㅓ = ㅝ
        (13, 5) => Some(15), // ㅜ + ㅔ = ㅞ
        (13, 20) => Some(16), // ㅜ + ㅣ = ㅟ
        (18, 20) => Some(19), // ㅡ + ㅣ = ㅢ
        _ => None,
    }
}

fn split_compound_vowel(vowel: u8) -> Option<u8> {
    match vowel {
        9 | 10 | 11 => Some(8),
        14 | 15 | 16 => Some(13),
        19 => Some(18),
        _ => None,
    }
}

fn combine_jong(first: u8, second: u8) -> Option<u8> {
    match (first, second) {
        (1, 19) => Some(3),   // ㄱㅅ
        (4, 22) => Some(5),   // ㄴㅈ
        (4, 27) => Some(6),   // ㄴㅎ
        (8, 1) => Some(9),    // ㄹㄱ
        (8, 16) => Some(10),  // ㄹㅁ
        (8, 17) => Some(11),  // ㄹㅂ
        (8, 19) => Some(12),  // ㄹㅅ
        (8, 25) => Some(13),  // ㄹㅌ
        (8, 26) => Some(14),  // ㄹㅍ
        (8, 27) => Some(15),  // ㄹㅎ
        (17, 19) => Some(18), // ㅂㅅ
        _ => None,
    }
}

fn split_compound_jong(jong: u8) -> Option<(u8, u8)> {
    match jong {
        3 => Some((1, 19)),
        5 => Some((4, 22)),
        6 => Some((4, 27)),
        9 => Some((8, 1)),
        10 => Some((8, 16)),
        11 => Some((8, 17)),
        12 => Some((8, 19)),
        13 => Some((8, 25)),
        14 => Some((8, 26)),
        15 => Some((8, 27)),
        18 => Some((17, 19)),
        _ => None,
    }
}

fn jong_to_choseong(jong: u8) -> Option<u8> {
    match jong {
        1 => Some(0),
        2 => Some(1),
        4 => Some(2),
        7 => Some(3),
        8 => Some(5),
        16 => Some(6),
        17 => Some(7),
        19 => Some(9),
        20 => Some(10),
        21 => Some(11),
        22 => Some(12),
        23 => Some(14),
        24 => Some(15),
        25 => Some(16),
        26 => Some(17),
        27 => Some(18),
        _ => None,
    }
}
