import React, { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("[ErrorBoundary] Caught error in page content:", error, errorInfo);
  }

  handleReload = () => {
    this.setState({ hasError: false, error: null });
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="page" role="alert">
          <div className="fatal">
            <h1 className="fatal__title">Something in the interface broke.</h1>
            <p className="fatal__text">
              The engine keeps running in the background. Reloading the window usually puts the
              interface back.
            </p>
            {this.state.error && <pre className="code">{this.state.error.message}</pre>}
            <div>
              <button type="button" className="btn btn--primary" onClick={this.handleReload}>
                Reload the window
              </button>
            </div>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
