import { Component, type ReactNode } from "react";

/** What `ErrorBoundary` shows: its `children`, or `failed` once they throw. */
export interface ErrorBoundaryProps {
  failed: ReactNode;
  children: ReactNode;
}

/**
 * Shows `failed` instead of `children` once rendering them throws, such as a suspending read
 * whose request failed, so the failure stays in its own part of the page.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, { caught: boolean }> {
  public constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { caught: false };
  }

  public static getDerivedStateFromError() {
    return { caught: true };
  }

  public override render(): ReactNode {
    return this.state.caught ? this.props.failed : this.props.children;
  }
}
