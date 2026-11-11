// The package ships no types. These are the options and methods the player uses, from its README.
declare module "@jellyfin/libass-wasm" {
  interface Options {
    video: HTMLVideoElement;
    subContent: string;
    fonts: string[];
    fallbackFont: string;
    workerUrl: string;
    onReady: () => void;
    onError: (error: unknown) => void;
  }

  export default class SubtitlesOctopus {
    constructor(options: Options);
    dispose(): void;
  }
}
