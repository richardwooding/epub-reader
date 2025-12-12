import { CSSProperties, useEffect, useRef } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

interface IframeViewerProps {
  uri: string;
  title?: string;
  width?: string | number;
  height?: string | number;
  style?: CSSProperties;
  className?: string;
}

interface EpubLinkMessage {
  type: string;
  url: string;
}

function IframeViewer({
  uri,
  title = "Content Viewer",
  width = "100%",
  height = "100%",
  style,
  className,
}: IframeViewerProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  // Control iframe src via useEffect (more reliable than JSX src prop for custom protocols)
  useEffect(() => {
    if (iframeRef.current) {
      iframeRef.current.src = uri;
    }
  }, [uri]);

  // Listen for postMessage from iframe to handle external links
  useEffect(() => {
    function handleMessage(event: MessageEvent) {
      // Validate message structure
      if (!event.data || typeof event.data !== 'object') {
        return;
      }

      const message = event.data as EpubLinkMessage;

      // Check message type
      if (message.type !== 'epub-external-link') {
        return;
      }

      // Validate URL exists
      if (!message.url || typeof message.url !== 'string') {
        console.error('[IframeViewer] Invalid external link message:', message);
        return;
      }

      // Validate origin (only accept from epub:// protocol or null origin)
      if (event.origin !== 'null' && !event.origin.startsWith('epub://')) {
        console.warn('[IframeViewer] Rejected message from origin:', event.origin);
        return;
      }

      // Open external link in system browser
      handleExternalLink(message.url);
    }

    async function handleExternalLink(url: string) {
      try {
        console.log('[IframeViewer] Opening external link:', url);
        await openUrl(url);
      } catch (error) {
        console.error('[IframeViewer] Failed to open external link:', error);
        alert(`Failed to open link: ${url}\n\nError: ${error}`);
      }
    }

    // Add event listener
    window.addEventListener('message', handleMessage);

    // Cleanup
    return () => {
      window.removeEventListener('message', handleMessage);
    };
  }, []);

  return (
    <iframe
      ref={iframeRef}
      title={title}
      width={width}
      height={height}
      style={style}
      className={className}
      frameBorder="0"
      sandbox="allow-same-origin allow-scripts allow-forms"
    />
  );
}

export default IframeViewer;
