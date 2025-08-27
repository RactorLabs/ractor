import type {ReactNode} from 'react';
import clsx from 'clsx';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  icon: string;
  description: ReactNode;
};

const FeatureList: FeatureItem[] = [
  {
    title: 'Simplest Developer Experience',
    icon: '⚡',
    description: (
      <>
        <strong>Deploy like serverless functions</strong> - just <code>npm install -g @raworc/cli</code> and start building.
        No infrastructure knowledge needed, no DevOps complexity, no vendor lock-in.
      </>
    ),
  },
  {
    title: 'Any Framework, Any Language',
    icon: '🚀',
    description: (
      <>
        <strong>Framework-agnostic</strong> runtime supporting Python, Node.js, and Rust.
        Deploy LangChain, CrewAI, AutoGen, LangGraph, or custom agents with just a <code>raworc.json</code> manifest.
      </>
    ),
  },
  {
    title: 'Production Ready from Day One',
    icon: '🏗️',
    description: (
      <>
        <strong>Enterprise features built-in</strong> - JWT auth, RBAC, encrypted secrets, session persistence.
        Scale from prototype to production without rebuilding infrastructure.
      </>
    ),
  },
  {
    title: 'Full Computer Access',
    icon: '🖥️',
    description: (
      <>
        Agents have access to <strong>filesystem operations</strong>, <strong>web browsing</strong>, and <strong>system tools</strong>.
        Secure containerized environments enable computer-use tasks safely.
      </>
    ),
  },
  {
    title: 'Session Persistence',
    icon: '💾',
    description: (
      <>
        <strong>Pause, save, and resume</strong> complex workflows. Never lose context or start over.
        Perfect for long-running tasks and iterative development.
      </>
    ),
  },
  {
    title: 'Zero Infrastructure Overhead',
    icon: '🎯',
    description: (
      <>
        <strong>Focus on agent logic</strong>, not DevOps. Professional deployment, monitoring, and operations 
        without the complexity. <strong>Deploy and go</strong>.
      </>
    ),
  },
];

function Feature({title, icon, description}: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center">
        <div className={styles.featureIcon}>{icon}</div>
      </div>
      <div className="text--center padding-horiz--md">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): ReactNode {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
