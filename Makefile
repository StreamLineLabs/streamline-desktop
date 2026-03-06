.PHONY: build test dev clean help install lint fmt

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'

install: ## Install dependencies
	npm install

dev: ## Start Tauri dev mode (hot-reload)
	npm run dev

build: ## Production build (creates installer)
	npm run build

preview: ## Preview the Vite build
	npm run preview

lint: ## Run linting checks
	cd src-tauri && cargo clippy --all-targets -- -D warnings

fmt: ## Check code formatting
	cd src-tauri && cargo fmt --all -- --check

clean: ## Clean build artifacts
	rm -rf dist node_modules/.vite
	cd src-tauri && cargo clean
