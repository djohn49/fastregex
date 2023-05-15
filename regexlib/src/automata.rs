use unic_ucd_category::GeneralCategory;

use crate::parser::character_class::CharacterClass;
use crate::parser::RegexEntry;

pub struct Automata {
    states: Vec<State>,
    terminal_states: Vec<usize>,
    start_state: usize,
}

pub struct State {
    pub debug_name: String,
    pub id: usize,
    pub transitions: Vec<Transition>,
}

pub struct Transition {
    pub next_state_id: usize,
    pub condition: TransitionCondition,
}

#[derive(Debug)]
pub enum TransitionCondition {
    AnyCharacter,
    Literal(char),
    CharacterClass(CharacterClass),
    UnicodeCharacterCLass(Vec<GeneralCategory>),
    NegatedUnicodeClass(Vec<GeneralCategory>),
    Epsilon,
}

impl Automata {
    fn new_empty() -> Self {
        Self {
            states: Vec::new(),
            terminal_states: Vec::new(),
            start_state: 0,
        }
    }

    pub fn from_regex(regex: RegexEntry) -> Self {
        let mut automata = Self::new_empty();

        let terminal_state_id = automata.add_state(State {
            debug_name: "terminal".into(),
            id: 0,
            transitions: vec![Transition::new(
                automata.next_state_id(),
                TransitionCondition::AnyCharacter,
            )],
        });

        automata.terminal_states.push(terminal_state_id);

        let start_state = automata.add_regex_entry(&regex, terminal_state_id);
        automata.start_state = start_state;

        automata
    }

    fn add_regex_entry(&mut self, entry: &RegexEntry, target: usize) -> usize {
        match entry {
            RegexEntry::AnyCharacter => self.construct_state(
                "AnyCharacter",
                [Transition::new(target, TransitionCondition::AnyCharacter)],
            ),
            RegexEntry::UnicodeCharacterClass(classes) => self.construct_state(
                "CharacterClass",
                [Transition::new(
                    target,
                    TransitionCondition::UnicodeCharacterCLass(classes.clone()),
                )],
            ),
            RegexEntry::NegatedUnicodeCharacterClass(classes) => self.construct_state(
                "NegatedUnicodeCharacterClass",
                [Transition::new(
                    target,
                    TransitionCondition::NegatedUnicodeClass(classes.clone()),
                )],
            ),
            RegexEntry::NonUnicodeCharacterClass(class) => self.construct_state(
                "NonUnicodeCharacterClass",
                [Transition::new(
                    target,
                    TransitionCondition::CharacterClass(class.clone()),
                )],
            ),
            RegexEntry::Literal(char) => self.construct_state(
                "Literal",
                [Transition::new(target, TransitionCondition::Literal(*char))],
            ),
            RegexEntry::Concatenation(entries) => {
                let mut last_target = target;
                for child_entry in entries.iter().rev() {
                    last_target = self.add_regex_entry(child_entry, last_target);
                }
                last_target
            }
            RegexEntry::Alternation(entries) => {
                let mut start_states = entries
                    .iter()
                    .map(|child_entry| self.add_regex_entry(child_entry, target))
                    .collect::<Vec<_>>();

                let mut start_state = self.construct_state(
                    "Alternation Epsilon Trampoline State",
                    start_states
                        .into_iter()
                        .map(|target| Transition::new(target, TransitionCondition::Epsilon)),
                );

                start_state
            }
            RegexEntry::Repetition {
                base,
                min: Some(min),
                max: Some(max),
            } => {
                //create accept states (accept within the repetition)
                let mut new_target = target;
                for _ in 0..((*max - *min) + 1) {
                    let epsilon_trampoline = self.construct_state(
                        "Repetition Epsilon Trampoline",
                        [
                            Transition::new(new_target, TransitionCondition::Epsilon),
                            Transition::new(target, TransitionCondition::Epsilon),
                        ],
                    );
                    new_target = self.add_regex_entry(base, epsilon_trampoline);
                }

                //create non-accept states (accept within the repetition)
                for _ in 0..(*min) {
                    new_target = self.add_regex_entry(base, new_target);
                }

                new_target
            }
            RegexEntry::Repetition {
                base,
                min: Some(min),
                max: None,
            } => {
                //looping repetition trampoline
                let epsilon_trampoline = self.construct_state(
                    "Repetition No-Maximum Epsilon Trampoline",
                    [Transition::new(target, TransitionCondition::Epsilon)],
                );

                //looping repetition implementation
                let loop_start = self.add_regex_entry(base, epsilon_trampoline);

                //wire back repetition trampoline in a loop
                self.states[epsilon_trampoline]
                    .transitions
                    .push(Transition::new(loop_start, TransitionCondition::Epsilon));

                //non-accept states
                let mut next_target = loop_start;
                if (*min != 0) {
                    for _ in 0..(*min - 1) {
                        next_target = self.add_regex_entry(base, next_target);
                    }
                }

                next_target
            }
            RegexEntry::Repetition {
                base,
                min: None,
                max: Some(max),
            } => {
                let mut next_target = target;
                for _ in 0..(*max) {
                    let epsilon_trampoline = self.construct_state(
                        "Repetition No-Minimum Epsilon Trampoline",
                        [Transition::new(next_target, TransitionCondition::Epsilon)],
                    );

                    next_target = self.add_regex_entry(base, epsilon_trampoline);

                    self.states[epsilon_trampoline]
                        .transitions
                        .push(Transition::new(target, TransitionCondition::Epsilon));
                }

                next_target
            }
            RegexEntry::Repetition {
                base,
                min: None,
                max: None,
            } => panic!(
                "Encountered repetition with no minimum or maximum. This is an internal error."
            ),
        }
    }

    fn construct_state(
        &mut self,
        name: impl Into<String>,
        transitions: impl IntoIterator<Item = Transition>,
    ) -> usize {
        self.add_state(State {
            id: 0,
            debug_name: name.into(),
            transitions: transitions.into_iter().collect(),
        })
    }

    fn add_state(&mut self, mut state: State) -> usize {
        let id = self.states.len();
        state.id = id;
        self.states.push(state);
        id
    }

    fn next_state_id(&self) -> usize {
        self.states.len()
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn get_state(&self, state_id: usize) -> &State {
        &self.states[state_id]
    }

    pub fn is_terminal_state(&self, state_id: usize) -> bool {
        self.terminal_states.contains(&state_id)
    }

    pub fn start_state_id(&self) -> usize {
        self.start_state
    }
}

impl Transition {
    fn new(next_state_id: usize, condition: TransitionCondition) -> Self {
        Self {
            next_state_id,
            condition,
        }
    }
}
