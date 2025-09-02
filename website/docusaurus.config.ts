import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: 'Raworc',
  tagline: 'Remote Agentic Work Orchestrator',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  // Set the production url of your site here
  url: 'https://raworc.com',
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: '/',

  // GitHub pages deployment config.
  // If you aren't using GitHub pages, you don't need these.
  organizationName: 'Raworc', // GitHub org name
  projectName: 'raworc-web', // This website repo name

  onBrokenLinks: 'warn',
  onBrokenMarkdownLinks: 'warn',

  // Even if you don't use internationalization, you can use this field to set
  // useful metadata like html lang. For example, if your site is Chinese, you
  // may want to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          // Edit links removed for Community Edition approach
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    // Replace with your project's social card
    image: 'img/logo.png',
    navbar: {
      title: 'Raworc',
      logo: {
        alt: 'Raworc Logo',
        src: 'img/logo.png',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'tutorialSidebar',
          position: 'left',
          label: 'Documentation',
        },
        {
          to: '/docs/api/api-overview',
          label: 'API Reference',
          position: 'left',
        },
        {
          to: '/docs/getting-started',
          label: 'Get Started',
          position: 'right',
        },
        {
          href: 'https://x.com/raworc',
          label: 'X',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentation',
          items: [
            {
              label: 'Getting Started',
              to: '/docs/',
            },
            {
              label: 'Computer Use Agents',
              to: '/docs/concepts/computer-use-agents',
            },
            {
              label: 'Sessions',
              to: '/docs/concepts/sessions',
            },
            {
              label: 'API Reference',
              to: '/docs/api/api-overview',
            },
          ],
        },
        {
          title: 'Community',
          items: [
            {
              label: 'X',
              href: 'https://x.com/raworc',
            },
          ],
        },
        {
          title: 'Resources',
          items: [
            {
              label: 'Get Started',
              to: '/docs/getting-started',
            },
            {
              label: 'CLI Usage Guide',
              to: '/docs/guides/cli-usage',
            },
            {
              label: 'Dev Mode',
              to: '/docs/guides/dev-mode',
            },
            {
              label: 'REST API',
              to: '/docs/api/rest-api-reference',
            },
          ],
        },
      ],
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'bash', 'json', 'yaml', 'sql'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
