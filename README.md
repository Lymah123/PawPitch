# PawPitch

A small local tool for new UGC (user-generated content) pet creators. It drafts outreach messages to pet brands with a local open-weight model (Gemma 3 4B via Ollama), checks every claim against the creator's own facts, and keeps a simple follow-up tracker.

## The idea: the model drafts, the code checks

1. Your facts live in `data/profile.json`. Empty fields stay empty.
2. For a brand, paste its description (or a gig post) into a text file.
3. Gemma drafts an email or DM and cites the facts it used. Brand sentences must be copied exactly; profile citations are advisory, because a small model paraphrases.
4. Rust rejects the draft if:
   - a cited phrase is not word for word in the sources;
   - it contains a number that is in neither source;
   - it claims followers, views or past brand partnerships, or equipment and experience your profile does not list;
   - it uses he or she for your pet when the profile does not state the pet's gender;
   - it names a content format your profile does not list (a profile that says "unboxing photos" cannot become "unboxing videos");
   - it says your pet has already tried or loved the brand's product. A proposal ("I would love to create...") is allowed;
   - it calls you or the pet something like "mom" or "owner" that your profile does not state.
   Soft personality words (adventures, loving, sweet...) are not rejected; they are listed as "double-check" warnings.
   The subject line is written by the code, so the model cannot embellish it.
   A draft that is rejected is regenerated with a new seed, up to `--tries` times (default 3).
   Words in the draft that appear nowhere in your facts are listed as "double-check" warnings.
5. You read, edit and send every message yourself. Nothing is sent automatically.

## Setup
    ollama pull gemma3:4b
    cargo build
    cargo run -- pitch --profile examples/sample-profile.json --brand "Wagwell Treats" --brand-file examples/sample-brand.txt

For your own use, copy `examples/sample-profile.json` to `data/profile.json` and fill it in with true facts only. `data/` is gitignored because it holds personal information.

## Check any pitch
    cargo run -- check --profile examples/sample-profile.json --brand "Wagwell Treats" --brand-file examples/sample-brand.txt --draft-file examples/bad-draft.txt
Fact-checks a pitch that PawPitch did not write (yours, or one from another chatbot) against your facts and the brand text. `examples/bad-draft.txt` is a real draft that an early version of this tool let through.

## Follow-up tracker
    cargo run -- log --brand "Wagwell Treats" --channel email
    cargo run -- due                 # who needs a follow-up after 7 days
    cargo run -- followed-up --brand "Wagwell Treats"
    cargo run -- status --brand "Wagwell Treats" --set replied
    cargo run -- list

## Evaluation
`./eval/run.sh` runs the fictional sample creator against five fictional brands in `examples/brands/` and prints PASS (with the attempt number) or FAIL for each. Raw output is saved in `eval/out/`. A FAIL means no draft in five attempts passed the fact check, which is the tool refusing to hand over something it cannot support.

## Limits
- It drafts; it cannot create demand. Many beginners get few replies at first.
- The fact check catches invented facts, numbers and a short list of claims. It does not judge tone or whether a brand is a good fit.
- Gig-post analysis (payment, usage rights, red flags) is not built yet.
- This is not legal or financial advice.

## License
MIT