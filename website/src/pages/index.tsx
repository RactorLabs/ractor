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
          Remote Agentic Work Orchestrator - Computer use agents with dedicated computers for each session. Intelligent agents that use computers like humans do to automate any manual work.
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
                <h3>Web Automation</h3>
                <p>Automate browser tasks, form filling, data extraction, and web workflows with natural language commands.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>Document Processing</h3>
                <p>Process files, generate reports, manipulate spreadsheets, and handle document workflows automatically.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>System Administration</h3>
                <p>Manage servers, deploy applications, monitor systems, and perform DevOps tasks through conversational interfaces.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>Data Analysis</h3>
                <p>Analyze datasets, create visualizations, run statistical models, and generate insights with full computer access.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>Software Development</h3>
                <p>Code generation, testing, debugging, and deployment with access to IDEs, terminals, and development tools.</p>
              </div>
              <div className={styles.useCaseCard}>
                <h3>Custom Workflows</h3>
                <p>Build specialized agents for any manual work - from CRM management to content creation and beyond.</p>
              </div>
            </div>
          </div>
        </section>
        <section style={{"backgroundColor": "#000000", "padding": "4rem 0"}}>
          <div className="container">
            <div style={{"textAlign": "center", "marginBottom": "3rem"}}>
              <Heading as="h2" style={{"color": "#ffffff"}}>What Computer Use Agents Can Do</Heading>
              <p style={{"fontSize": "1.2rem", "color": "#cccccc", "maxWidth": "800px", "margin": "0 auto"}}>
                Computer Use Agents can automate any manual work that involves using a computer
              </p>
            </div>
            <div style={{"display": "grid", "gridTemplateColumns": "repeat(auto-fit, minmax(300px, 1fr))", "gap": "2rem", "maxWidth": "1200px", "margin": "0 auto"}}>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>🤖 Build Agentic AI Products</h3>
                <p style={{"color": "#cccccc"}}>Develop intelligent agents using LangGraph, CrewAI, AutoGen with conversational interfaces</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>⚡ Supercharge Make/n8n Workflows</h3>
                <p style={{"color": "#cccccc"}}>Extend automation platforms with computer use capabilities for complex tasks</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>📊 Generate Stunning Reports</h3>
                <p style={{"color": "#cccccc"}}>Create professional reports with charts, analysis, and formatted presentations</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>🎨 Create Great Presentations</h3>
                <p style={{"color": "#cccccc"}}>Build compelling slide decks, infographics, and visual content automatically</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>🌐 Operate Remote Browsers</h3>
                <p style={{"color": "#cccccc"}}>Control web browsers for testing, data extraction, and web automation</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>🔍 Research Any Topic</h3>
                <p style={{"color": "#cccccc"}}>Deep web research, fact-checking, and comprehensive information gathering</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>📈 Conduct Market Research</h3>
                <p style={{"color": "#cccccc"}}>Analyze competitors, track trends, and generate market intelligence reports</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>💻 Write Better Code</h3>
                <p style={{"color": "#cccccc"}}>Generate, review, and optimize code with AI-powered development assistance</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>🧪 Perform Quality Assurance Testing</h3>
                <p style={{"color": "#cccccc"}}>Automated testing of applications, websites, and user workflows</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>📱 Generate Content for Social Media</h3>
                <p style={{"color": "#cccccc"}}>Create posts, images, videos, and manage social media content pipelines</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>💰 Handle Financial Operations</h3>
                <p style={{"color": "#cccccc"}}>Process invoices, reconcile accounts, and manage financial data workflows</p>
              </div>
              <div style={{"textAlign": "center", "padding": "1.5rem", "backgroundColor": "#1a1a1a", "borderRadius": "8px", "boxShadow": "0 2px 4px rgba(255,255,255,0.1)", "border": "1px solid #333"}}>
                <h3 style={{"color": "#007bff", "marginBottom": "1rem"}}>📝 Fill Out Boring Forms</h3>
                <p style={{"color": "#cccccc"}}>Automate form filling, data entry, and repetitive administrative tasks</p>
              </div>
            </div>
          </div>
        </section>
        <section className={styles.runtimeSection}>
          <div className="container">
            <div style={{"textAlign": "center", "maxWidth": "800px", "margin": "0 auto"}}>
              <Heading as="h2">Remote Computer Use Agents</Heading>
              <p style={{"fontSize": "1.2rem", "marginBottom": "2rem"}}>
                Raworc offers <strong>Computer use agents with dedicated computers for each session</strong>. 
                Intelligent Host that uses computers like humans do - browsing, managing files, running software 
                through <strong>natural language commands</strong>.
              </p>
              <Link
                className="button button--primary button--lg"
                to="/docs/concepts/computer-use-agents">
                Learn About Computer Use Agents →
              </Link>
            </div>
          </div>
        </section>
        <section className={styles.developmentSection}>
          <div className="container">
            <p className={styles.sectionIntro}>
              Raworc offers Computer use agents with dedicated computers that automate manual work 
              just like human workers - with full software access and natural language control interfaces.
            </p>
            <div className={styles.developmentGrid}>
              <div className={styles.developmentCard}>
                <h3>🚀 Instant Computer Use Agents</h3>
                <p>
                  <strong>Get Computer use agents with dedicated computers</strong> in seconds. 
                  No setup required - intelligent Host ready to automate any manual work with full computer access.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>🔄 Persistent Work Sessions</h3>
                <p>
                  <strong>Never lose progress</strong> - close agent sessions and restore them later with all files, state, and context preserved. 
                  Perfect for long-running automation tasks.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>🧪 Isolated Automation Environments</h3>
                <p>
                  <strong>Each session provides Computer use agents with dedicated computers</strong> and full OS access. 
                  Safe to run any automation without affecting your local machine.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>📊 Enterprise Ready</h3>
                <p>
                  <strong>Production-grade automation</strong> - RBAC, encrypted secrets, audit trails, and monitoring. 
                  Deploy computer-use agents at scale with enterprise security.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>⚡ Natural Language Control</h3>
                <p>
                  <strong>Control computers with conversation</strong> - no APIs, SDKs, or complex integrations. 
                  Just describe what you want automated and the agent does it.
                </p>
              </div>
              <div className={styles.developmentCard}>
                <h3>🤝 Scalable Automation</h3>
                <p>
                  <strong>Run multiple Computer use agents</strong> simultaneously with dedicated computers and proper access controls. 
                  Share automation sessions, fork workflows, and scale manual work elimination.
                </p>
              </div>
            </div>
            <div style={{"textAlign": "center", "marginTop": "3rem"}}>
              <h3>Stop Doing Manual Work. Start Automating.</h3>
              <p style={{"fontSize": "1.1rem", "marginBottom": "2rem"}}>
                Join teams already using Raworc to automate manual work with computer-use agents
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
        <section style={{"backgroundColor": "#000000", "padding": "2rem 0", "textAlign": "center", "borderTop": "1px solid #333"}}>
          <div className="container">
            <p style={{"margin": "0", "color": "#cccccc", "fontSize": "0.9rem"}}>
              Made with ❤️ by <a href="https://remoteagent.com" target="_blank" rel="noopener noreferrer" style={{"color": "#007bff", "textDecoration": "none"}}>RemoteAgent team</a>
            </p>
          </div>
        </section>
      </main>
    </Layout>
  );
}
