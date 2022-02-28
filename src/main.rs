use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::io::Write;
use std::iter::zip;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;
use std::thread;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum CharacterStatus {
    CORRECT,
    INCORRECT,
    EXISTS
}

#[derive(Debug)]
struct CharacterState {
    character: char,
    status: CharacterStatus
}

#[derive(Debug)]
struct GuessResult {
    char_states: [CharacterState; 5],
    exists: [i8; 26]
}

impl CharacterState {
    fn new(character: char) -> CharacterState {
        CharacterState{
            character,
            status: CharacterStatus::INCORRECT
        }
    }
}

fn to_chars<T>(x: T) -> [char; 5] where T: AsRef<str> {
    let mut it = x.as_ref().chars();
    return [
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
    ];
}

fn to_string(chars: [char; 5]) -> String {
    return chars.iter().map(|x| x.to_string()).collect::<Vec<String>>().join("");
}

fn get_possible_words() -> Result<Vec<[char; 5]>, Box<dyn Error>> {
    let words = fs::read_to_string("/Users/tanmaybakshi/wordle/possible_words.txt")?
        .split("\n")
        .filter(|x| x.len() == 5)
        .map(|x| to_chars(x))
        .collect();
    return Ok(words);
}

fn get_allowed_words() -> Result<Vec<[char; 5]>, Box<dyn Error>> {
    let words = fs::read_to_string("/Users/tanmaybakshi/wordle/allowed_words.txt")?
        .split("\n")
        .filter(|x| x.len() == 5)
        .map(|x| to_chars(x))
        .collect();
    return Ok(words);
}

fn get_word_frequencies() -> Result<HashMap<[char; 5], i64>, Box<dyn Error>> {
    let frequencies: Vec<([char; 5], i64)> = fs::read_to_string("/Users/tanmaybakshi/wordle/unigram_freq.csv")?
        .split("\n")
        .filter(|x| x.contains(","))
        .map(|x| {
            let mut components = x.split(",").map(|x| x.to_string());
            let word = components.next().unwrap();
            let _frequency = components.next().unwrap();
            let frequency: i64 = _frequency.parse().unwrap();
            return (word, frequency);
        })
        .filter(|x| x.0.len() == 5)
        .map(|x| (to_chars(x.0), x.1))
        .collect();
    let mut freq_map = HashMap::new();
    for (word, freq) in frequencies {
        freq_map.insert(word, freq);
    }
    return Ok(freq_map);
}

fn get_states(guess: &[char; 5], real: &[char; 5]) -> GuessResult {
    let mut exists: [i8; 26] = [0; 26];
    let mut known_exists: [i8; 26] = [0; 26];
    let mut char_states: [CharacterState; 5] = [
        CharacterState::new(guess[0]),
        CharacterState::new(guess[1]),
        CharacterState::new(guess[2]),
        CharacterState::new(guess[3]),
        CharacterState::new(guess[4]),
    ];
    for (index, (guess_char, real_char)) in zip(guess, real).enumerate() {
        if guess_char == real_char {
            char_states[index].status = CharacterStatus::CORRECT;
        } else {
            exists[*real_char as usize - 97] += 1;
        }
    }
    for index in 0..5 {
        if char_states[index].status == CharacterStatus::CORRECT {
            continue;
        }
        let c = guess[index] as usize - 97;
        if exists[c] > 0 {
            known_exists[c] += 1;
            exists[c] -= 1;
            char_states[index].status = CharacterStatus::EXISTS;
        } else if known_exists[c] > 0 {
            known_exists[c] = -known_exists[c];
        }
    }
    for index in 0..5 {
        let c = guess[index] as usize - 97;
        if char_states[index].status == CharacterStatus::INCORRECT && known_exists[c] == 0 {
            known_exists[c] = -6;
        }
    }
    GuessResult{
        char_states,
        exists: known_exists
    }
}

fn word_is_valid(word: &[char; 5], states: &GuessResult) -> bool {
    let mut exists: [i8; 26] = [0; 26];
    for i in 0..5 {
        let c = word[i] as usize - 97;
        if states.char_states[i].status == CharacterStatus::CORRECT {
            if word[i] != states.char_states[i].character {
                return false;
            }
            exists[c] += 1;
            continue;
        }
        if states.char_states[i].status == CharacterStatus::EXISTS && word[i] == states.char_states[i].character {
            return false;
        }
        if states.exists[c] == -6 {
            return false;
        }
        exists[c] += 1;
    }
    let mut i = 0;
    for (ge, re) in zip(exists, states.exists) {
        i += 1;
        if re == -6 || re == 0 {
            continue;
        }
        let is_capped = re < 0;
        let total = if is_capped { -re } else { re };
        if (is_capped && ge != total) || (!is_capped && ge < total) {
            return false;
        }
    }
    return true;
}

fn get_valid_words(words: Vec<[char; 5]>, states: GuessResult) -> Vec<[char; 5]> {
    words
        .into_iter()
        .filter(|word| word_is_valid(&word, &states))
        .collect()
}

fn total_valid_words(words: &Vec<[char; 5]>, states: GuessResult) -> usize {
    words
        .iter()
        .filter(|word| word_is_valid(&word, &states))
        .count()
}

fn get_expected_value(guess: &[char; 5], words: &Vec<[char; 5]>) -> f32 {
    let mut values = HashMap::new();
    let mut total_words = 0;
    for word in words {
        if guess == word {
            continue;
        }
        total_words += 1;
        let result = get_states(guess, word);
        let value = total_valid_words(&words, result);
        *values.entry(value).or_insert(0) += 1;
    }
    let mut avg_value = 0.0_f32;
    for (value, occ) in values {
        let probability = (occ as f32) / (total_words as f32);
        avg_value += (value as f32) * probability;
    }
    return avg_value;
}

fn wordle_worker(guess_recv: Receiver<[char; 5]>, words: Vec<[char; 5]>, value_send: Sender<([char; 5], f32)>) -> Result<(), Box<dyn Error>> {
    loop {
        let guess = guess_recv.recv()?;
        let value = get_expected_value(&guess, &words);
        value_send.send((guess, value))?;
    }
}

fn create_guess_result(chars: [(char, CharacterStatus); 5]) -> GuessResult {
    let mut exists: [i8; 26] = [0; 26];
    let mut exists_formatted: [i8; 26] = [0; 26];
    for (c, s) in &chars {
        let idx = *c as usize - 97;
        if s != &CharacterStatus::INCORRECT {
            exists[idx] += 1;
        }
    }
    for (c, s) in &chars {
        let idx = *c as usize - 97;
        if exists[idx] == 0 {
            exists_formatted[idx] = -6;
        } else {
            if s == &CharacterStatus::INCORRECT {
                exists_formatted[idx] = -exists[idx];
            } else {
                exists_formatted[idx] = exists[idx];
            }
        }
    }
    GuessResult{
        char_states: [
            CharacterState{
                character: chars[0].0,
                status: chars[0].1
            },
            CharacterState{
                character: chars[1].0,
                status: chars[1].1
            },
            CharacterState{
                character: chars[2].0,
                status: chars[2].1
            },
            CharacterState{
                character: chars[3].0,
                status: chars[3].1
            },
            CharacterState{
                character: chars[4].0,
                status: chars[4].1
            },
        ],
        exists: exists_formatted
    }
}

fn gr<T>(word: T, colors: T) -> GuessResult where T: AsRef<str> {
    let wchars = to_chars(word);
    let cchars = to_chars(colors);
    let mut states: [(char, CharacterStatus); 5] = [('a', CharacterStatus::INCORRECT); 5];
    for i in 0..5 {
        states[i].0 = wchars[i];
        states[i].1 = match cchars[i] {
            'B' => CharacterStatus::INCORRECT,
            'Y' => CharacterStatus::EXISTS,
            'G' => CharacterStatus::CORRECT,
            _ => unreachable!()
        };
    }
    return create_guess_result(states);
}

fn guess_pass() {
    let mut allowed_words = get_allowed_words().unwrap();
    let mut possible_words = get_possible_words().unwrap();
    let word_freqs = get_word_frequencies().unwrap();

    let args: Vec<String> = env::args().skip(1).collect();
    let word_count = args.len() / 2;
    for i in 0..word_count {
        let (word, result) = (&args[i * 2], &args[i * 2 + 1]);
        allowed_words = get_valid_words(allowed_words, gr(word, result));
        possible_words = get_valid_words(possible_words, gr(word, result));
    }

    let mut word_senders = Vec::new();
    let (value_sender, value_receiver) = channel();
    for _ in 0..10 {
        let (guess_sender, guess_receiver) = channel();
        word_senders.push(guess_sender);
        let wc = possible_words.clone();
        let vc = value_sender.clone();
        thread::spawn(move|| {
            wordle_worker(guess_receiver, wc, vc);
        });
    }

    let mut i = 0;
    for word in &allowed_words {
        word_senders[i].send(word.clone());
        i = (i + 1) % word_senders.len();
    }

    let mut next = Vec::new();
    loop {
        let (word, value) = value_receiver.recv().unwrap();
        next.push((word, value));
        print!("{} / {}\r", next.len(), allowed_words.len());
        io::stdout().flush();
        if next.len() == allowed_words.len() {
            break;
        }
    }

    next.sort_by(|x, y| {
        if y.1 != x.1 {
            return y.1.partial_cmp(&x.1).unwrap();
        }
        let y_possible = possible_words.contains(&y.0);
        let x_possible = possible_words.contains(&x.0);
        if y_possible != x_possible {
            let yp = if y_possible { 1 } else { 0 };
            let xp = if x_possible { 1 } else { 0 };
            return xp.partial_cmp(&yp).unwrap();
        }
        let y_freq = word_freqs.get(&y.0).or(Some(&0)).unwrap();
        let x_freq = word_freqs.get(&x.0).or(Some(&0)).unwrap();
        return x_freq.partial_cmp(&y_freq).unwrap();
    });
    for (word, value) in next {
        println!("{} -> {}", to_string(word), value);
    }
}

fn main() {
    guess_pass();
}
