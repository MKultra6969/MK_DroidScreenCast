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
        <div className="app-backdrop" aria-hidden />
        <div className="flex min-h-screen items-center justify-center p-6">
          <div className="m3-dialog m3-dialog--enter max-w-[520px] text-center">
            <h1 className="m3-headline-small">Something went wrong</h1>
            <p className="mt-2 m3-body-medium m3-on-variant">
              The UI hit an unexpected error. Reload to try again.
            </p>
            {this.state.error?.message && (
              <pre className="m3-code mt-3 max-h-40 text-left">
                {this.state.error.message}
              </pre>
            )}
            <button
              className="m3-btn m3-state m3-btn--filled m3-btn--sm mt-4"
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
