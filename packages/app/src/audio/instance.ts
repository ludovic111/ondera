import { WebAudioEngine } from './engine';

/** The one engine for this renderer. Created here so components can audition notes without prop drilling. */
export const engine = new WebAudioEngine();
