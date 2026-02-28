# Contributing to Streamline Desktop

Thank you for your interest in contributing to Streamline Desktop! This guide will help you get started.

## Getting Started

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes
4. Run tests and linting
5. Commit your changes (`git commit -m "Add my feature"`)
6. Push to your fork (`git push origin feature/my-feature`)
7. Open a Pull Request

## Prerequisites

- Node.js 18 or later
- Rust 1.80 or later
- Platform-specific dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`
  - **Windows**: WebView2 runtime

## Development Setup

```bash
# Clone your fork
git clone https://github.com/<your-username>/streamline-desktop.git
cd streamline-desktop

# Install dependencies
npm install

# Start development server
npm run dev

# Build production binary
npm run build
```

## Code Style

- Follow TypeScript conventions for frontend code
- Follow Rust conventions for Tauri backend code
- Use meaningful variable and function names
- Keep components focused and composable

## Pull Request Guidelines

- Write clear commit messages
- Add tests for new functionality
- Update documentation if needed
- Ensure `npm run build` passes before submitting

## Reporting Issues

- Use the **Bug Report** or **Feature Request** issue templates
- Search existing issues before creating a new one
- Include your OS and version for platform-specific bugs

## Code of Conduct

All contributors are expected to follow our [Code of Conduct](https://github.com/streamlinelabs/.github/blob/main/CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions will be licensed under the Apache-2.0 License.
