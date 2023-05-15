use std::collections::{BTreeMap, BTreeSet};
use std::process::id;

use unic_ucd_category::GeneralCategory;

use crate::parser::character_class::CharacterClass;
use crate::parser::RegexEntry;

#[derive(Clone)]
pub struct Automata {
    states: Vec<State>,
    terminal_states: Vec<usize>,
    start_states: Vec<usize>,
}

#[derive(Clone)]
pub struct State {
    pub debug_name: String,
    pub id: usize,
    pub transitions: Vec<Transition>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Transition {
    pub next_state_id: usize,
    pub condition: TransitionCondition,
}

#[derive(Debug, Clone, Eq, PartialEq)]
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
            start_states: Vec::new(),
        }
    }

    pub fn simplify(&mut self) {
        self.remove_duplicate_transitions();
        self.simplify_states();
        self.remove_dead_states();
    }

    fn remove_dead_states(&mut self) {
        let mut new_states = self
            .states
            .iter()
            .filter(|state| !self.is_state_dead(*state))
            .map(|state| state.clone())
            .collect::<Vec<_>>();

        let id_map = new_states
            .iter()
            .enumerate()
            .map(|(index, state)| (state.id, index))
            .collect::<BTreeMap<_, _>>();

        for new_state in &mut new_states {
            new_state.id = id_map[&new_state.id];
            new_state.transitions = new_state
                .transitions
                .iter()
                .filter_map(|transition| {
                    Some(Transition::new(
                        *id_map.get(&transition.next_state_id)?,
                        transition.condition.clone(),
                    ))
                })
                .collect();
        }

        self.start_states = self
            .start_states
            .iter()
            .filter_map(|state_id| id_map.get(state_id))
            .map(|u| *u)
            .collect();

        self.terminal_states = self
            .terminal_states
            .iter()
            .filter_map(|state_id| id_map.get(state_id))
            .map(|u| *u)
            .collect();

        self.states = new_states;
    }

    fn is_state_dead(&self, state: &State) -> bool {
        let mut checked = BTreeSet::new();
        self.is_state_dead_checked(&mut checked, state)
    }

    fn is_state_dead_checked(&self, checked: &mut BTreeSet<usize>, state: &State) -> bool {
        checked.insert(state.id);

        if self.terminal_states.contains(&state.id) {
            return false;
        }

        for transition in &state.transitions {
            if !checked.contains(&transition.next_state_id) {
                if !self.is_state_dead_checked(checked, &self.states[transition.next_state_id]) {
                    return false;
                }
            }
        }

        true
    }

    fn simplify_states(&mut self) {
        let mut new_start_states = BTreeSet::new();
        for start_state in &self.start_states {
            let mut epsilon_reach = BTreeSet::new();
            self.calculate_epsilon_reach(&mut epsilon_reach, *start_state);
            new_start_states.append(&mut epsilon_reach);
        }
        self.start_states = new_start_states.into_iter().collect();

        self.states = self
            .states
            .iter()
            .map(|state| self.calculate_new_state(state))
            .collect();
    }

    fn calculate_new_state(&self, old: &State) -> State {
        let mut state = State {
            id: old.id,
            debug_name: old.debug_name.clone(),
            transitions: Vec::new(),
        };

        for old_transition in &old.transitions {
            if !old_transition.condition.is_epsilon() {
                let mut target_epsilon_reach = BTreeSet::new();
                self.calculate_epsilon_reach(
                    &mut target_epsilon_reach,
                    old_transition.next_state_id,
                );

                for new_target in target_epsilon_reach {
                    state.transitions.push(Transition::new(
                        new_target,
                        old_transition.condition.clone(),
                    ));
                }
            }
        }

        state
    }

    fn calculate_epsilon_reach(&self, set: &mut BTreeSet<usize>, state_id: usize) {
        if !set.contains(&state_id) {
            set.insert(state_id);
            for transition in &self.states[state_id].transitions {
                if let TransitionCondition::Epsilon = transition.condition {
                    self.calculate_epsilon_reach(set, transition.next_state_id);
                }
            }
        }
    }

    fn remove_duplicate_transitions(&mut self) {
        for state in &mut self.states {
            let mut new_transitions = Vec::new();
            for transition in &state.transitions {
                if !new_transitions.contains(transition) {
                    new_transitions.push(transition.clone());
                }
            }
            state.transitions = new_transitions;
        }
    }

    pub fn from_regex(regex: RegexEntry) -> Self {
        let mut automata = Self::new_empty();

        let terminal_state_id = automata.add_state(State {
            debug_name: "terminal".into(),
            id: 0,
            transitions: vec![],
        });

        automata.terminal_states.push(terminal_state_id);

        let start_state = automata.add_regex_entry(&regex, terminal_state_id);
        automata.start_states = vec![start_state];

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
                min,
                max: Some(max),
            } => {
                //create accept states (accept within the repetition)
                let mut new_target =
                    self.construct_maximum_repetition_count(target, base, *max - *min);

                //create non-accept states (accept within the repetition)
                new_target = self.construct_exact_repetition_count(new_target, base, *min);

                new_target
            }
            RegexEntry::Repetition {
                base,
                min,
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
                let non_accept_start =
                    self.construct_exact_repetition_count(epsilon_trampoline, base, *min);

                non_accept_start
            }
        }
    }

    fn construct_exact_repetition_count(
        &mut self,
        target: usize,
        base: &RegexEntry,
        count: u64,
    ) -> usize {
        let mut new_target = target;
        for _ in 0..count {
            new_target = self.add_regex_entry(base, new_target);
        }
        new_target
    }

    fn construct_maximum_repetition_count(
        &mut self,
        target: usize,
        base: &RegexEntry,
        max: u64,
    ) -> usize {
        let mut last_target = target;
        for _ in 0..max {
            let this_iteration_start = self.add_regex_entry(base, last_target);
            last_target = self.construct_state(
                "Maximum Repetition Count Epsilon Trampoline",
                [
                    Transition::new(this_iteration_start, TransitionCondition::Epsilon),
                    Transition::new(target, TransitionCondition::Epsilon),
                ],
            )
        }
        last_target
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

    pub fn start_states(&self) -> &[usize] {
        &self.start_states
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

impl TransitionCondition {
    fn is_epsilon(&self) -> bool {
        match self {
            TransitionCondition::Epsilon => true,
            _ => false,
        }
    }
}
