set shell := ["bash", "-c"]
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set working-directory := '..'

[private]
default:
    @just --list agent

[doc("Re-ingest all documentation into the agent knowledge base.")]
ingest:
    mise exec -- python3 backend/agent/scripts/ingest-docs.py
