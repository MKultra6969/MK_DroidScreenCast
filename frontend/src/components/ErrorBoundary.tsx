import { Component, type ErrorInfo, type ReactNode } from 'react';

type ErrorBoundaryProps = {
  children: ReactNode;
};

type ErrorBoundaryState = {
  hasError: boolean;
  error?: Error;
};

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = {
    hasError: false
  };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('ui crash', error, info);
  }

  handleReload = () => {
    window.location.reload();
  };

  render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    return (
      <div className="min-h-screen">
        <div className="background">
          <div className="orb orb-1" />
          <div className="orb orb-2" />
          <div className="grid-overlay" />
        </div>
        <div className="flex min-h-screen items-center justify-center p-6">
          <div className="w-full max-w-[520px] rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] p-6 text-center shadow-[var(--shadow-3)]">
            <h1 className="font-display text-xl font-semibold">Something went wrong</h1>
            <p className="mt-2 text-sm text-[var(--md-sys-color-on-surface-variant)]">
              The UI hit an unexpected error. Reload to try again.
            </p>
            {this.state.error?.message && (
              <pre className="mt-3 max-h-40 overflow-auto rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] p-3 text-left text-xs text-[var(--md-sys-color-on-surface-variant)]">
                {this.state.error.message}
              </pre>
            )}
            <button
              className="mt-4 inline-flex items-center justify-center rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]"
              type="button"
              onClick={this.handleReload}
            >
              Reload
            </button>
          </div>
        </div>
      </div>
    );
  }
}
