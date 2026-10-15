# PawPitch

A small local tool for new UGC (user-generated content) pet creators. It drafts outreach messages to pet brands with a local open-weight model (Gemma 3 4B via Ollama), checks every claim against the creator's own facts, and keeps a simple follow-up tracker.

## The idea: the model drafts, the code checks

1. Your facts live in `data/profile.json`. Empty fields stay empty.
2. For a brand, paste its description (or a gig post) into a text file.
3. Gemma drafts an email or DM and must cite the exact phrases it used from your profile and from the brand text.
4. Rust rejects the draft if a cited phrase is not word for word in the sources, if it contains a number that is in neither source, or if it claims things like followers, views or past brand partnerships that your profile does not mention.
5. You read, edit and send every message yourself. Nothing is sent automatically.

## Setup
    ollama pull gemma3:4b
    cargo build
    cargo run -- pitch --profile examples/sample-profile.json --brand-file examples/sample-brand.txt

For your own use, copy `examples/sample-profile.json` to `data/profile.json` and fill it in with true facts only. `data/` is gitignored because it holds personal information.

## Follow-up tracker
    cargo run -- log --brand "Wagwell Treats" --channel email
    cargo run -- due                 # who needs a follow-up after 7 days
    cargo run -- followed-up --brand "Wagwell Treats"
    cargo run -- status --brand "Wagwell Treats" --set replied
    cargo run -- list

## Limits
- It drafts; it cannot create demand. Many beginners get few replies at first.
- The fact check catches invented facts, numbers and a short list of claims. It does not judge tone or whether a brand is a good fit.
- Gig-post analysis (payment, usage rights, red flags) is not built yet.
- This is not legal or financial advice.

## License
MIT
