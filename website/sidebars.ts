import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

/**
 * Creating a sidebar enables you to:
 - create an ordered group of docs
 - render a sidebar for each doc of that group
 - provide next/previous navigation

 The sidebars can be generated from the filesystem, or explicitly defined here.

 Create as many sidebars as you want.
 */
const sidebars: SidebarsConfig = {
  // Main documentation sidebar
  tutorialSidebar: [
    {
      type: 'doc',
      id: 'intro',
      label: 'Introduction',
    },
    {
      type: 'doc',
      id: 'getting-started',
      label: 'Getting Started',
    },
    {
      type: 'category',
      label: 'Core Concepts',
      link: {
        type: 'generated-index',
        title: 'Core Concepts',
        description: 'Understand Computer use agents and automation capabilities',
      },
      items: [
        'concepts/computer-use-agents',
        'concepts/agents',
        'concepts/agent-names-and-publishing',
        'concepts/computer-use-landscape',
        'concepts/authentication-users',
      ],
    },
    {
      type: 'category',
      label: 'User Guides',
      link: {
        type: 'generated-index',
        title: 'User Guides',
        description: 'Learn how to use Raworc CLI and automate manual work',
      },
      items: [
        'guides/cli-usage',
        'guides/dev-mode',
        'guides/agent-playground',
      ],
    },
    {
      type: 'category',
      label: 'API Reference',
      link: {
        type: 'generated-index',
        title: 'API Reference',
        description: 'Complete REST API documentation for Raworc',
      },
      items: [
        'api/api-overview',
        'api/rest-api-reference',
      ],
    },
  ],
};

export default sidebars;
