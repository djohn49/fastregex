# Experimental Regular Expression Matching Engine via Rust Procedural Macros For Gains In Performance and Simplicity
## Introduction
### Motivation
In Rust, the library that is used in the vast majority of cases for regular expressions is regex¹. Unlike most regular expression libraries which use recursion and backtracking, this library uses an automata (NFAs and DFAs, depending on the case) to implement each regular expression². This has the advantage that matching time on a string for a given regular expression scales linearly with the string’s length in the worst case. Additionally, this library is highly optimized from a software engineering and micro-optimization, as opposed to algorithmic, perspective.

Overall, it is a very high quality regular expression library, however it has some notable limitations. These limitations come from the design choice to compile regular expressions into automata at compile time. Consider the following example code from regex’s documentation:

    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    assert!(re.is_match("2014-01-01"));

First, a Regex object is created using the Regex::new constructor, which parses the given regular expression at runtime and then emits an automata, also at runtime. Then, this automata is stored in a data structure which can then be read when matching on a particular string. Additionally, a particular Regex object has internal scratch space that is used for lazy evaluation of some information (such as DFA states which are dynamically lowered from an NFA during matching), which causes locking contention if a single Regex object is used from multiple threads (or loss of shared caching if separate Regex objects are used).
### Project
Rust has a system, procedural macros, which would seem to be able to alleviate some of these concerns. Procedural macros allow one to create a library that exposes functions that run at compile time and transforms a piece of the token stream into another piece. Much has been written about Rust’s procedural macros in general, and a good overview can be found on Youtube³. As such, this project is to create a procedural macro that takes as input a regular expression string and then, at compile time, automatically generates bespoke Rust code that implements that particular regular expression. Internally, this macro works via these five basic steps:
1. Tokenize the regular expression into tokens such as a character class like [A-Z], opening and closing parenthesis, and character literals
2. Parse the tokens into an abstract syntax tree of the regular expression
3. Lower the regular expression into an NFA
4. Simplify the generated NFA
5. Emit generated Rust code that implements the simplified NFA
Additionally, there is a stand-alone CLI executable which integrates with graphviz⁴ to output a .svg of a NFA, both before and after simplification. Lastly, a lot of these pieces (such as regex parsing and automaton generation) could be re-used from the existing regex implementation, but this project is in large part educational and it is more educational (and interesting) to rewrite these things. Some libraries and existing tools were, however, used: syn⁵ and quote⁶ for procedural macro utilities, criterion⁷ for benchmarking, graphviz⁸ for graph generation, itertools⁹ for some string manipulation utilities for graphviz code generation, and unic-ucd-category¹⁰ and unic-char-property¹¹ for Unicode support (these libraries provide a Unicode character property database). These tools were used because the provided tasks are not primarily related to regular expressions but are nonetheless useful in this project.

## Implementation Details
Each of the following sections covers, in moderate deteail, how one part of this regular expression engine works. The tokenizer, parser, automata generator, and automata simplification system are in regexlib. Regexlib is a reusable library that is then consumed both by fastregex, a procedural macro library and nfadiagram, a standalone executable project that generates a graphviz representation of the simplified and unsimplified NFA for a given regular expression. Overall, the project takes 2017 lines of code as measured by cloc¹².

### Tokenization
The first step to turn a regular expression into a (Rust) implementation is to parse it and the first step in parsing is tokenization. In this parsing system, the tokenizer tokenizes as much as possible that can be consumed linearly (i.e., without looking ahead or behind), as the code is implemented by a series of functions that try to parse a token from the beginning of the string, and then if successful they return the parsed token and the remaining string. This is inspired by the idea of a parser combinator¹³, which is usually used to parse a context-free grammar, however in this case there is no well-defined context-free grammar. The tokens are as follows: AnyCharacter (. in regex), UnicodeCharacterClasss (parsed from sequences such as \pL and \p{Upercase_Letter}, and also from \d for all digits), NegatedUnicodeCharacterClass (the same as UnicodeCharacterClass but negated), NonUnicodeCharacterClasss (which parses tokens such as [A-Za-z0-9]), Alternation (||), OpenGroup ( ( ), CloseGroup ( ) ), Repetition ({0,1}, and operations such as a klein star and + (at least once) are simplified to a repetition in the tokenizer), and literal (simply a character that is matched literally). An example of the data that the tokenizer inputs and outputs can be found in the test_tokenize unit test in regexlib/src/lib/parser/tokenizer.rs in the provided source code. Most of the parsing work is done in the lexer, which is 718 lines of code as compared to the rest of the parser’s 508 lines of code (both of these counts include tests).

### Parsing
Once the regular expression is lexed into a sequence of tokens, it must be parsed into an abstract syntax tree. While it would be possible to use a standard parser technique such as a parser combinator or a LR parser, these techniques have requirements on the sort of grammar that is used (any language may be matched but the grammar for the particular language that is usable may not be the most convenient). As such, a different technique is used. Specifically, an intermediate representation is created that can represent a partially parsed abstract syntax tree at each stage of parsing. The representation is a vector of the following emum (enums in Rust are a sum type, or a tagged union):

    #[derive(PartialEq, Eq, Debug, Clone)]
    enum PartiallyParsed {
        Lexed(RegexToken),
        Group(Vec<PartiallyParsed>),
        Repetition {
            base: Box<PartiallyParsed>,
            min: u64,
            max: Option<u64>,
        },
        Alternation(Vec<PartiallyParsed>),
    }

A series of operations (grouping, repetitions, and alternations) are performed on this data structure, which is then lowered to a final AST right before that final AST is simplified. 	Precedence order is determined by the order of the parsing steps, so, for example, repetitions have a higher precedence than alternations.

The first step in parsing is to lex, so the lexer is run and the output is turned into a vector of PartiallyParsed::Lexed. Then, a recursive function goes over this vector of lexed tokens and groups them based on parenthesis. This grouping system works by searching for the first open group and then calling itself to consume that group when it encounters it, and then returns (when it encounters a close group token or the end of the input) a group of partially parsed tokens where any found groups are grouped.

The remaining parsing steps operate on a linear (i.e., ungrouped) series of partially parsed items where groups are considered a single item. Thus, the function parse_for_all_groups_recursively takes another function as a parameter to do this linear parsing and then recurses over the groups in the partially parsed AST to perform that modification on all levels. This function is used for repetition and alternation parsing.

Repetition parsing works by finding all instances of the Repetition token (Recall that this token refers both to explicit repetitions such as {5, 6} and implicit repetitions such as ? which desugars to {0, 1}. This simplification is done in the lexer.), and then creating a PartiallyParsed::Repetition item with the Repetition token and the preceding item. Due to the parse_for_all_groups_recursively function this operates at all layers of the AST.

Alternation parsing is more complex, and is not ideal. It is possible to do this operation without repeated searching, but as the regular expressions in question are unlikely to be large and because parsing is not the focus of this project a simpler repeated-searching algorithm is used. This algorithm repeatedly searches for the first alternation token and then combines the preceding and succeeding items into an alternation. If the preceding item is already an alternation, then the succeeding item is instead simply added to that alternation in order to cleanly represent alternations of more than two items. Recursively representing such alternations as nested binary alternations would work but then a simple algorithm for NFA generation would generate extraneous states that then would translate to worse runtime performance. Note that the generated AST at the end of the parsing process is, strictly speaking, not an AST due to the use of non-binary alternations. However, the language is equivalent and the pseudo-AST is used like an AST so this distinction is not meaningful for this project and thus this representation is considered to be an AST in the rest of this document.

Lastly, to finalize parsing and lower to the final AST representation, remaining unparsed tokens (literals, character classes, unicode character classes, and negated unicode character classes) are simply converted directly to AST entries. The AST is then simplified by removing groups and concatenations of length 1 and replacing these with the single entry.

An example of what the parser outputs is given in the following unit test (found in regexlib/src/parser/mod.rs):

    #[test]
    fn test_complex_parse_1() {https://en.wikipedia.org/wiki/Parser_combinator
        use unic_ucd_category::GeneralCategory::*;
        use RegexEntry::*;
        test_full_parse(
            r#"((\d\PL)*){1,3}"#,
            Repetition {
                base: Box::new(Repetition {
                    base: Box::new(Concatenation(vec![
                        UnicodeCharacterClass(vec![DecimalNumber, OtherNumber, LetterNumber]),
                        NegatedUnicodeCharacterClass(vec![
                            UppercaseLetter,
                            LowercaseLetter,
                            TitlecaseLetter,
                            ModifierLetter,
                            OtherLetter,   
                        ]),
                    ])),
                    min: 0,
                    max: None,
                }),
                min: 1,
                max: Some(3),
            },
        );
    }

### NFA Generation
#### Data Structure
Now that a regular expression can be parsed, the NFA can be considered. The first step here is to create a data structure to represent the NFA. We use the following structure (defined in regexlib/src/automata.rs):

    pub struct Automaton {
        states: Vec<State>,
        terminal_states: Vec<usize>,
        start_states: Vec<usize>,
        prefix: String,
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
    
    pub enum TransitionCondition {
        AnyCharacter,
        Literal(char),
        CharacterClass(CharacterClass),
        UnicodeCharacterClass(Vec<GeneralCategory>),
        NegatedUnicodeClass(Vec<GeneralCategory>),
        Epsilon,
    }

There is an Automaton struct which represents a single automaton for a single specific regular expression. It contains a list of states, the terminal states (by index into the list of states), the start states (also by id), and a prefix.

The de-facto standard regex crate in Rust has an optimization where regular expressions with literal prefixes first check this prefix before going into automaton-based checking, since prefix checking is faster. Additionally, by not considering the prefix as part of the regular expression the NFA can have fewer states, which allows the final Rust code that implements this automaton to, in theory, be faster due to smaller jump tables or smaller state structs (described in NFA implementation).

A state is represented by a debug name, the id (index into the states array), and a list of transitions. The transitions have the id of the state that the transition targets (which could be the state that it is part of) and a condition in which the transition takes place. The transition condition is represented as an enum, which is Rust’s mechanism for sum types (and is thus more powerful than enums in other languages). Transition conditions are: transition on any character, transition on a specific character, transition when matching a non-unicode character class (such as [A-Z] or [^0-9]), transition when matching any unicode character class, transition when not matching any unicode character class, and epsilon transitions.

#### Initial NFA Generation
NFA generation is found in regexlib/src/automata.rs and the entry point is the function from_regex. The algorithm works by first creating an empty automaton (i.e., no states and an empty prefix) and then adding onto it. The first addition is to create a terminal state and add it to the terminal states vector. Then, there is a recursive function called add_regex_entry. This function takes a single RegexEntry (a vertex/node in the AST, which is itself represented by an instance of this struct that refers to the root node) and a target state id, and it returns the initial generated state. Specifically, it generates a series of states to implement the specific RegexEntry and returns the state index that serves as the entry point to that series of states. This can then be run on the root of the AST, which contains the entire regular expression where the returned state is the start state of the automaton.

To generate the states for any character, a state is simply added with an unconditional transition to the target state. That state’s id is returned as the start state. A similar strategy is used for unicode and non-unicode character classes and literals, except the transition is conditional as to implement the required state.

Concatenations work by iterating over the entries in the concatenation and setting the target state of entry n to the returned start state of n+1, or the target state of the concatenation in the case of the last entry. The returned start state of the first entry in the concatenation is returned as the start state of the concatenation.

Alternations work by creating a start state which points, with an epsilon transition, to the start state of every entry in the alternation. Each entry in the alternation points directly to the target state.

Repetitions are done in two cases: where there is a maximum, and where there is not. Not having a minimum is equivalent to having a minimum of zero, so this does not need to be specifically considered. Recall that operators such as ?, +, and * desugar to repetitions. Repetitions without a maximum simply emit the minimum number of repetitions which then are connected to an infinite loop that can exit or loop back after any repetition via epsilon transitions. Repetitions with a maximum emit the minimum number of repetitions and then the maximum number minus the minimum number, where epsilon transitions permit leaving the target after any repetition after the minimum number.

<img src="https://raw.githubusercontent.com/djohn49/fastregex/master/readmeassets/complex.svg">

Figure 1: The regular expression `https?://(([A-Za-z.]+/)+([A-Za-z.]+)?)|([A-Za-z.]+)`, lowered to an un-simplified NFA.

#### NFA Simplification

Epsilon transitions are not supported in the code that generates an NFA implementation in Rust, however they are very useful for NFA generation from an AST. Thus, during NFA simplification epsilon transitions must be simplified out. Additionally, a literal prefix is checked for, after which the start state is changed. Lastly, once epsilon transitions are removed and the start state is moved some states are no longer reachable from the start state and some states can no longer reach the end state.

The first step is to find if there is a prefix. To do this, the transitions out of the start state are considered. If there is one transition and it is a literal, then the character from that literal is added to the prefix and the start state is changed to the target of that transition. This process is repeated until there is not exactly one transition, or the one transition is not a literal. This process can’t run if there is more than one start state, so it must run before epsilon transition simplification.

Next, duplicated transitions are removed. That is, if a single state has two identical transitions, one of them is removed.

Next, epsilon transitions are removed. This is done separately for the start states and other states. For start states, the epsilon reach of every existing start state is found, and then the union of those epsilon reaches is taken. This union is then the new set of start states. Then, for every non-epsilon transition on every other state, the epsilon reach of the target state is counted and the transition condition is duplicated once for each entry in the epsilon reach of the target state. Epsilon transitions are also removed during this step.

Lastly, dead states are removed. First, a depth-first search is performed from every start state and the union of the results of all reachable states is found. Any state that is outside of this union is removed. Then, for each state, a depth-first search is performed from that state that terminates upon reaching any terminal state. If no terminal state is reached, then that starting state of this search is removed.

<img src="https://raw.githubusercontent.com/djohn49/fastregex/master/readmeassets/simple.svg">

Figure 2: The NFA for the regular expression `https?://(([A-Za-z.]+/)+([A-Za-z.]+)?)|([A-Za-z.]+)` after simplification. See figure 1 for the automaton before simplification.

### NFA Implementation
Two implementation strategies were created in order to try more attempts to increase performance. First, a boolean-based system was created, but with it performance was generally worse than the existing de-facto standard regex crate. Another system was created as an attempt to remedy this, which unfortunately had very similar performance. How each of these systems work is described in the next subsections. The boolean system is not present in the latest commit in the provided code, but can be viewed in commit 9ba29476b940ea62ea04a14398b285a68e564867. Note that the code that is described in this section (for both the boolean and enum list case) is automatically generated from a state machine.

#### Booleans
The basic idea for this system is for each implementation to emit a struct called Automaton that represents a particular DFA state when the NFA is lowered to a DFA. More precisely, the Automaton struct has a boolean member variable for each NFA state that is true if and only if the corresponding NFA state is true. See the Automaton¹⁴ struct for the previously-used URL matcher regex:

    struct Automoton {
        state0: bool,
        state1: bool,
        state2: bool,
        state3: bool,
        state4: bool,
        state5: bool,
        state6: bool,
        state7: bool,
        state8: bool,
        state9: bool,
        state10: bool,
        state11: bool,
        state12: bool,
        state13: bool,
        state14: bool,
    }

Then, there is an advance function which takes as input an Automaton instance and a character, and then outputs an Automaton instance that represents the next DFA state. This works by considering all incoming transitions to each state and then creating a disjunction between the boolean expressions for when that incoming transition takes place. For example, state 2 (see figure 2) can be reached from either state 3 or state 6 and in both cases only if the next character is ‘/’. Thus, the expression to determine if state 2 is included in the next DFA sate is as follows:

    state2: (self.state3 && (next == '/')) || (self.state6 && (next == '/'))

Most states’ expressions are more complicated, however the follow the same basic pattern. See the following excerpt of the advance function’s code for the URL-matching regular expression:

    fn advance(&self, next: char) -> Self {
        Self {
            state0: (self.state1
                && ((((next as u32) >= ('A' as u32)) && ((next as u32) <= ('Z' as u32)))
                    || (((next as u32) >= ('a' as u32)) && ((next as u32) <= ('z' as u32)))
                    || (next == '.')))
                || (self.state2
                    && ((((next as u32) >= ('A' as u32)) && ((next as u32) <= ('Z' as u32)))
                        || (((next as u32) >= ('a' as u32)) && ((next as u32) <= ('z' as u32)))
                        || (next == '.')))
                || (self.state3 && (next == '/'))
                || (self.state6 && (next == '/'))
                || (self.state9
                    && ((((next as u32) >= ('A' as u32)) && ((next as u32) <= ('Z' as u32)))
                        || (((next as u32) >= ('a' as u32)) && ((next as u32) <= ('z' as u32)))
                        || (next == '.')))
                || (self.state10
                    && ((((next as u32) >= ('A' as u32)) && ((next as u32) <= ('Z' as u32)))
                        || (((next as u32) >= ('a' as u32)) && ((next as u32) <= ('z' as u32)))
                        || (next == '.'))),
            state1: (self.state1
                && ((((next as u32) >= ('A' as u32)) && ((next as u32) <= ('Z' as u32)))
                    || (((next as u32) >= ('a' as u32)) && ((next as u32) <= ('z' as u32)))
                    || (next == '.')))
                || (self.state2
                    && ((((next as u32) >= ('A' as u32)) && ((next as u32) <= ('Z' as u32)))
                        || (((next as u32) >= ('a' as u32)) && ((next as u32) <= ('z' as u32)))
                        || (next == '.'))),
    
            //expressions for states 2 through 9 removed for the sake of brevity
            state10: (self.state11 && (next == '/')),
            state11: (self.state12 && (next == '/')),
            state12: (self.state13 && (next == ':')),
            state13: (self.state14 && (next == 's')),
            state14: false,
        }
    }


Next, we have two more functions: is_terminated and is_failed. is_terminated returns if the state represented by the Automaton instance is a terminal state. That is, it returns true if and only if at least one of the terminal state’s members is true. The is_failed function simply returns true if all member states are false, which means that no terminal state may be reached (Any states that could not reach a terminal state were removed during NFA simplification.).

With these pieces, some basic boilerplate to step through the automaton for a string that is known at runtime can be emitted:

    let string = ::core::convert::AsRef::as_ref(&string);
    if string.len() < 4 {
        return false;
    }
    let (prefix, string) = string.split_at(4);
    if prefix != "http" {
        return false;
    }
    let mut chars = str::chars(string);
    let mut automaton = Automoton::new();
    while let Some(char) = chars.next() {
        automaton = automaton.advance(char);
        if (automaton.is_failed()) {
            return false;
        }
    }
    automaton.is_terminated()

First, some memory management boilerplate is used to get the string as a slice that can be iterated over. Then, if the string is shorter than the static required prefix it is known that it can’t match, so early return of false is possible. Then, the first n characters are split from the rest of the string where n is the length of the prefix. If the prefix of the string does not match the required prefix of the regular expression, early return of false is also possible. Note that one could generate an automaton that matches the static prefix as well, however simple character comparisons are faster. Furthermore, this optimization is present in the de-facto standard regex crate so replicating it here provides a more comparable benchmark. Then, an Automaton instance is instantiated that represents the start state (each boolean member variable is true if and only if the corresponding NFA state is a start state) and it is advanced for each character in the string. If, at any point, the automaton is failed as-per the is_failed function an early return of false is used to save time. Lastly, after processing the string returns true, the string matches the regular expression if and only if is_terminated returns true.

#### Enum Lists

The enum lists implementation strategy is very similar to the boolean-based strategy, but the internal representation of the Automaton is different. The basic idea is to store a list of current states instead of storing a true/false value for each state. The idea is that then states that are not currently active (which at any point is expected to be most states) do not need to be considered. The trade-off is that the logic to keep track of lists is more complicated. 

The first step in implementing such a list is representing states. This is done using an enum. Note that this enum is used like an enum in most programming languages (i.e., it represents on of some set of values) as opposed to as the more powerful sum type that is also available in Rust. See the enum for the URL regex:

    enum State {
        State0,
        State1,
        State2,
        State3,
        State4,
        State5,
        State6,
        State7,
        State8,
        State9,
        State10,
        State11,
        State12,
        State13,
        State14,
    }

The Automaton struct now needs to represent a list of currently active states. Naively, one might think that a vector type (Vec in Rust) is the best choice here, however this data structure performs heap allocations which is not necessary in this case. We know that at most every state can be present at once, so we only need space to store that many states. Of course, fewer states may be present. As such, a fixed-size array with a flag beside it to say how many entries of that array are valid (arbitrarily chosen to be the first n states where n is the number of valid states) is used:

    struct Automaton {
        states: [State; 15],
        valid_state_count: usize,
    }

Now, a new form of the advance function is required. The basic idea is to initialize the states array to contain garbage data and valid_state_count to be zero, and then fill it in. However, initializing the state array takes linear time with respect to the length of the underlying array and the goal of this system is to not have the time scale with the number of states in the NFA. Thus, a trick is used where two instances of Automaton are created in the control flow and where one overwrites itself to contain the next state from another. Additionally, some fixed-length scratch space is required (described later). The new control flow (after prefix checking) looks like this:

    let mut scratch_space = ScratchSpace::new();
    let mut automaton_a = Automaton::new();
    let mut automaton_b = Automaton::new();
    let mut from_automaton = &mut automaton_a;
    let mut to_automaton = &mut automaton_b;
    while let Some(char) = chars.next() {
        to_automaton.advance_from(from_automaton, char, &mut scratch_space);
        if (to_automaton.is_failed()) {
            return false;
        }
        ::core::mem::swap(to_automaton, from_automaton);
    }
    to_automaton.is_terminated()

Within the advance function, the first step is to set the current valid_state_count to zero. Then, the previous states are iterated over. A match statement (which is essentially as switch statement in this case) jumps to code that considers all of the outgoing edges for each previous state. If an outgoing edge matches the current character, then the valid_state_count variable is incremented and the state is inserted into the state array.

However, there is an edge case to consider. If two or more previous states both trigger the same next state, then that state will be added to the array twice. To prevent this, we need some way to check if a particular state was already added to the array. We could iterate over the array to check directly, however then the worst case matching performance is O(n2) with the length of the string, instead of O(n) as with the previous implementation. To resolve this problem, an array is used where each index contains whether or not the state corresponding to that index has already been added. Of course, re-initializing this array to all false (or using a new array) for each new character that is processed defeats the intended goal of this implementation strategy. As such, an array of integers, rather than booleans, is used. We call this array scratch space, as briefly mentioned earlier. At the start of matching this array’s value is initialized to all zerores. Then, to signify that a state has been added in the nth character the corresponding entry in the array is set to n+1. Then, a state has been added to the state array if and only if the corresponding entry in the scratch space entry is equal to n+1.

Overall, the advance function looks like this (some parts are removed for brevity):

    pub fn advance_from(&mut self, from: &Automaton, next: char, scratch: &mut ScratchSpace) {
        scratch.did_add_state_value += 1;
        self.valid_state_count = 0;
        for from_state in from.states.iter().take(from.valid_state_count) {
            match from_state {
                State::State0 => {}
                State::State1 => {
                    if (scratch.did_add_state[0usize] != scratch.did_add_state_value)
                        && ((((next as u32) >= ('A' as u32)) && ((next as u32) <= ('Z' as u32)))
                            || (((next as u32) >= ('a' as u32)) && ((next as u32) <= ('z' as u32)))
                            || (next == '.'))
                    {
                        scratch.did_add_state[0usize] = scratch.did_add_state_value;
                        self.states[self.valid_state_count] = State::State0;
                        self.valid_state_count += 1;
                    }
                    if (scratch.did_add_state[1usize] != scratch.did_add_state_value)
                        && ((((next as u32) >= ('A' as u32)) && ((next as u32) <= ('Z' as u32)))
                            || (((next as u32) >= ('a' as u32)) && ((next as u32) <= ('z' as u32)))
                            || (next == '.'))
                    {
                        scratch.did_add_state[1usize] = scratch.did_add_state_value;
                        self.states[self.valid_state_count] = State::State1;
                        self.valid_state_count += 1;
                    }
                }
    
                //states 2-12 removed for brevity
                State::State13 => {
                    if (scratch.did_add_state[12usize] != scratch.did_add_state_value) && (next == ':')
                    {
                        scratch.did_add_state[12usize] = scratch.did_add_state_value;
                        self.states[self.valid_state_count] = State::State12;
                        self.valid_state_count += 1;
                    }
                }
                State::State14 => {
                    if (scratch.did_add_state[13usize] != scratch.did_add_state_value) && (next == 's')
                    {
                        scratch.did_add_state[13usize] = scratch.did_add_state_value;
                        self.states[self.valid_state_count] = State::State13;
                        self.valid_state_count += 1;
                    }
                }
            }
        }
    }

## Benchmarking Results
In order to measure the performance of the experimental engines as compared to the de-facto standard regex crate, Criterion1 is used. Criterion describes itself as a library for “Statistics-driven Microbenchmarking”, which is exactly what is needed here since matching on reasonable string sizes in any of these engines is very fast (on the order of nanoseconds). Criterion runs the benchmark some number of times, on the order of tens of millions to billions. It also includes a warm-up cycle to, for example, prepare CPU caches and includes primitives to stop the compiler from creating unwanted optimizations (such as completely removing calls to a pure function with static input if the input would not be realistically static in a real-world use case).

For benchmarking, 15 scenarios are run with 5 strings to match against. In all cases, the URL matching regex from previous sections is used and it is matched against 5 different strings where some match and some do not where those strings are designed to exercise different parts of an engine. The de-facto standard regex crate was given an identical regular expression as the experiential engines, except it was prefixed with ^ and suffixed with $ in order to signify that the entire input should be matched rather than searching for the a match in any substring. This matches the behavior of the experimental engines without this prefix and suffix.

All benchmarks were compiled with rustc 1.69.0 (84c898d65 2023-04-16) in release mode and run on Fedora Linux 38 (Workstation Edition) x86_64 with a i9-9900k at stock clock speed with Linux kernel version 6.2.14-300.fc38.x86_64. Benchmark numbers for both the de-facto standard regex crate and for the experimental engines can be reproduced by running “cargo bench” in the provided code’s directory.

| Match Against | Regex Crate (ns) | Boolean Automaton (ns) | Enum List Automaton (ns) |
| - | - | - | - |
| http://test | 27.069 | 62.248 | 61.025 |
| http:/ | 23.529 | 19.751 | 17.105 |
| http:// | 25.653 | 27.985 | 24.011 |
| http://example.com/this/is/a/test/page.html | 64.763 | 319.41 | 338.18 |
| The quick brown fox jumped over the lazy dog. | 64.453 | 2.2977 | 2.2113 |
| Average | 41.0934 | 86.33834 | 88.50646 |

<img src="https://raw.githubusercontent.com/djohn49/fastregex/master/readmeassets/performancechart.png">

Unfortunately, the experimental engines are on average more than twice as slow as the de-facto standard. However, in some cases it does pull ahead. This shows that perhaps the technique is valid, but more refinement is needed. See the future work section for more details on this.

## Conclusion & Future Work

As mentioned in the benchmarking section, performance is not as good as it potentially could be as compared to the de-facto standard regex crate. Future work could involve attempting to track down why this is the case. One idea to improve performance are to use better NFA simplification (perhaps using the existing regex crate to generate NFAs) as there are some cases where the simplified NFA is not minimal. Another option is to fully lower to a DFA at compile-time, and represent each DFA state with its own enum. Lastly, a completely different technique, a backtracking recursive parser, could be used. This may be faster on some inputs because this code would more closely resemble most code that is written, so LLVM may be better-able to optimize it. However, backtracking engines have very poor worst-case algorithmic time complexity that can lead to “pathological behavior.”1 Lastly, instruction-level profiling can be used to try to find where most of the time is spent in the experimental engines in order to guide micro-optimization efforts. Some basic instruction-level profiling pointed towards cache misses as a possible culprit.

In addition to performance improvements, there is other future work that could be completed in this area. Specifically, it is desirable to have a more thorough test suite. It may be a good idea to reuse the test suite from the regex crate as part of this in order to test a new library as a drop-in replacement. Additionally, supporting a more complete set of regex is desirable. At the moment, Unicode character groups are incomplete and capturing groups are not supported either. Support for operations such as matching any part of a string rather than the entire string would also be useful and is necessary to create a more complex regex engine. Lastly, the regex parser as it is written right now is very non-ideal. Specifically, some parts of the parser are slower than they need to be for the operations that they do, and, more importantly, it is not cleanly organized as a specific context-free grammar and parser for that grammar.

Despite the failure to reliably beat the de-facto standard crate’s performance, some new information was learned. Specifically, the enum list and boolean-based systems perform very similarly, showing that the increased complexity of enum lists is probably not worth it. Additionally, to implement my own regex engine, even though it is not particularly fast, has been a very educational experience, which is the primary goal of such a project.

### Footnotes
1: https://crates.io/crates/regex

2: https://github.com/rust-lang/regex/blob/master/PERFORMANCE.md

3: https://www.youtube.com/watch?v=MWRPYBoCEaY

4: https://graphviz.org/ 

5: https://crates.io/crates/syn

6: https://crates.io/crates/quote

7: https://crates.io/crates/criterion

8: https://graphviz.org/

9: https://crates.io/crates/itertools

10: https://docs.rs/unic-ucd-category/

11: https://docs.rs/unic-char-property/

12: https://github.com/AlDanial/cloc

13: https://en.wikipedia.org/wiki/Parser_combinator

14: Note that the name is misspelled in this implementation. This error was only noticed and corrected when rewriting for the enum lists implementation.