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
    title: 'Instant Remote Computers',
    icon: '⚡',
    description: (
      <>
        <strong>Get Computer use agents with dedicated computers</strong> - just <code>npm install -g @raworc/cli</code> and start automating.
        No setup needed, full computer access, completely isolated environments.
      </>
    ),
  },
  {
    title: 'Computer Use Agents',
    icon: '🚀',
    description: (
      <>
        <strong>Agent runtime with dedicated computers</strong> - web browsing, file management, software installation, and system operations.
        Full access to terminals, browsers, IDEs, and any software through conversational interfaces.
      </>
    ),
  },
  {
    title: 'Automate Any Manual Work',
    icon: '🏗️',
    description: (
      <>
        <strong>Enterprise-grade automation</strong> - from data entry to system administration, agents can automate any task.
        Scale manual work automation from single tasks to complex multi-step workflows.
      </>
    ),
  },
  {
    title: 'Dedicated Remote Computers',
    icon: '🖥️',
    description: (
      <>
        Each agent provides <strong>a dedicated computer</strong> with full OS access.
        Perfect for automation tasks that require dedicated computing environments and intelligent execution.
      </>
    ),
  },
  {
    title: 'Persistent Agents',
    icon: '💾',
    description: (
      <>
        <strong>Never lose work progress</strong> — sleep long-running automation tasks and wake them later.
        <strong>Agent remix</strong> to branch automation workflows and try different approaches.
      </>
    ),
  },
  {
    title: 'Natural Language Interface',
    icon: '🎯',
    description: (
      <>
        <strong>Describe what you want automated</strong> in plain English. No coding required - 
        agents understand instructions and execute complex multi-step workflows.
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
