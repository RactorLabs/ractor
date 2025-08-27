import type {ReactNode} from 'react';
import React, { useState } from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import HomepageFeatures from '@site/src/components/HomepageFeatures';
import Heading from '@theme/Heading';

import styles from './index.module.css';

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  const [copied, setCopied] = useState(false);

  const copyCommand = async () => {
    console.log('Copy button clicked'); // Debug log
    
    const text = 'npm install -g @raworc/cli';
    let copySuccess = false;
    
    try {
      // Modern clipboard API
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
        copySuccess = true;
        console.log('Copied using modern API');
      } else {
        // Fallback for older browsers
        const textArea = document.createElement('textarea');
        textArea.value = text;
        textArea.style.position = 'fixed';
        textArea.style.opacity = '0';
        textArea.style.left = '-9999px';
        
        document.body.appendChild(textArea);
        textArea.focus();
        textArea.select();
        
        try {
          copySuccess = document.execCommand('copy');
          console.log('Copied using fallback method');
        } catch (fallbackErr) {
          console.error('Fallback copy failed:', fallbackErr);
        }
        
        document.body.removeChild(textArea);
      }
    } catch (err) {
      console.error('Copy failed:', err);
    }
    
    // Show feedback regardless of success (for UX)
    setCopied(true);
    setTimeout(() => {
      setCopied(false);
    }, 2000);
    
    if (copySuccess) {
      console.log('Copy successful!');
    }
  };

  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <div className={styles.heroLogo}>
          <img src="/img/logo.png" alt="Raworc Logo" width="150" />
        </div>
        <Heading as="h1" className="hero__title">
          {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <p className={styles.heroDescription}>
          The simplest way to deploy AI agents. Deploy any agent from any framework with the easiest developer experience in the industry.
        </p>
        
        <div className={styles.npmInstall}>
          <div className={styles.npmBox}>
            <pre>npm install -g @raworc/cli</pre>
            <button 
              className={styles.copyButton} 
              onClick={copyCommand}
              title={copied ? 'Copied!' : 'Copy to clipboard'}
            >
              {copied ? '✓' : 
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" stroke="currentColor" strokeWidth="2" fill="none"/>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" stroke="currentColor" strokeWidth="2" fill="none"/>
                </svg>
              }
            </button>
          </div>
          <p className={styles.dockerRequirement}>
            <small>* Requires Docker</small>
          </p>
        </div>
        
        <div className={styles.buttons}>
          <Link
            className="button button--primary button--lg"
            to="/docs/getting-started">
            Get Started →
          </Link>
        </div>
      </div>
    </header>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  
  return (
    <Layout
      title={`${siteConfig.title} - Universal AI Agent Runtime`}
      description="Raworc is the first universal AI agent runtime that solves the deployment gap between AI frameworks and production. Deploy any agent from any framework with enterprise-grade operations and zero dependencies.">
      <HomepageHeader />
      <main>
        <HomepageFeatures />
        <section className={styles.useCasesSection}>
          <div className="container">
            <div className={styles.useCasesGrid}>
              <div className={styles.useCaseCard}>
                <h3>LangChain Agents</h3>
                <p>Deploy LangChain RAG agents, chains, and tools with automatic dependency management and session persistence.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>CrewAI Teams</h3>
                <p>Run collaborative CrewAI multi-agent teams with intelligent delegation and shared workspace coordination.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>AutoGen Conversations</h3>
                <p>Execute Microsoft AutoGen conversational workflows with persistent state and context management.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>LangGraph Workflows</h3>
                <p>Deploy complex LangGraph state machines with pause/resume capabilities and data lineage tracking.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>BYOA (Bring Your Own Agent)</h3>
                <p>Bring any custom agent implementation - Python, Node.js, or Rust - with zero runtime dependencies.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>Runtime Flexibility</h3>
                <p>Mix frameworks within sessions, migrate between agents, and avoid vendor lock-in with universal runtime.</p>
              </div>
            </div>
          </div>
        </section>
        <section className={styles.runtimeSection}>
          <div className="container">
            <div style={{"textAlign": "center", "maxWidth": "800px", "margin": "0 auto"}}>
              <Heading as="h2">What is an Agent Runtime?</Heading>
              <p style={{"fontSize": "1.2rem", "marginBottom": "2rem"}}>
                An Agent Runtime is the missing infrastructure layer between AI frameworks and production deployment. 
                Just as web applications need servers and mobile apps need operating systems, 
                <strong> AI agents need runtimes</strong> to handle deployment, security, persistence, and coordination at scale.
              </p>
              <Link
                className="button button--primary button--lg"
                to="/docs/concepts/agent-runtime">
                Learn About Agent Runtimes →
              </Link>
            </div>
          </div>
        </section>
        <section className={styles.developmentSection}>
          <div className="container">
            <p className={styles.sectionIntro}>
              Raworc bridges the gap between AI agent development and production deployment, 
              eliminating the infrastructure complexity that slows teams down.
            </p>
            <div className={styles.developmentGrid}>
              <div className={styles.developmentCard}>
                <h3>🚀 Rapid Prototyping</h3>
                <p>
                  <strong>Start immediately</strong> with any framework - LangChain, CrewAI, AutoGen, or custom code. 
                  No infrastructure setup required. Just add a <code>raworc.json</code> and deploy.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>🔄 Iterative Development</h3>
                <p>
                  <strong>Session persistence</strong> lets you pause experiments, iterate on code, and resume exactly where you left off. 
                  No lost context or restarting from scratch.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>🧪 Safe Experimentation</h3>
                <p>
                  <strong>Containerized environments</strong> provide safe sandboxes for testing agent behavior. 
                  Experiment with file operations, web browsing, and system tools without risk.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>📊 Production Readiness</h3>
                <p>
                  <strong>Enterprise features built-in</strong> - RBAC, encrypted secrets, audit trails, and monitoring. 
                  Scale from prototype to production without rebuilding infrastructure.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>⚡ Zero Infrastructure Overhead</h3>
                <p>
                  <strong>Focus on agent logic</strong>, not DevOps. Pre-compiled dependencies, automatic scaling, 
                  and professional deployment pipelines handle the operational complexity.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>🤝 Team Collaboration</h3>
                <p>
                  <strong>Multi-tenant workspaces</strong> enable teams to collaborate on agents with proper access controls. 
                  Share sessions, fork experiments, and maintain development history.
                </p>
              </div>
            </div>
            <div style={{"textAlign": "center", "marginTop": "3rem"}}>
              <h3>Stop Fighting Infrastructure. Start Building Agents.</h3>
              <p style={{"fontSize": "1.1rem", "marginBottom": "2rem"}}>
                Join teams already using Raworc to accelerate their AI agent development
              </p>
              <div className={styles.buttons}>
                <Link
                  className="button button--primary button--lg"
                  to="/docs/getting-started">
                  Get Started →
                </Link>
                <Link
                  className="button button--outline button--primary button--lg"
                  to="/docs/">
                  View Documentation
                </Link>
              </div>
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
