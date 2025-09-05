# Raworc Documentation Website

This is the official documentation website for Raworc.

## Installation

```bash
npm install
```

## Local Development

```bash
npm start
```

This command starts a local development server and opens up a browser window. Most changes are reflected live without having to restart the server.

## Build

```bash
npm run build
```

This command generates static content into the `build` directory and can be served using any static contents hosting service.

## Deployment

### Using GitHub Pages:

```bash
GIT_USER=<Your GitHub username> npm run deploy
# or
USE_SSH=true yarn deploy
```

### Using other hosting services:

1. Build the site: `npm run build`
2. Deploy the `build` folder to your hosting service

## Documentation Structure

- `/docs` - Main documentation content
  - `/getting-started` - Installation and quick start guides
  - `/concepts` - Core concepts and architecture
  - `/guides` - User guides and tutorials
  - `/admin` - Administrator documentation (section not present yet)
  - `/api` - API reference
- `/blog` - Blog posts and announcements (blog disabled)
- `/src` - Website source code
  - `/components` - React components
  - `/pages` - Additional pages
  - `/css` - Custom styles

## Key Features

- 📚 Comprehensive documentation for Raworc
- 📱 Mobile-responsive design
- 🌙 Dark mode support
- 📊 API reference with examples
- 🚀 Quick start guides

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test locally with `npm start`
5. Submit a pull request

<!-- Search is currently disabled (Algolia DocSearch removed). -->

## License

This documentation follows the same unlicensed status as the Raworc project.
