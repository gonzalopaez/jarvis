type Listener<T> = (payload: T) => void;

export class EventBus<Events extends { [K in keyof Events]: unknown }> {
  private listeners = new Map<keyof Events, Set<Listener<Events[keyof Events]>>>();

  on<K extends keyof Events>(event: K, listener: Listener<Events[K]>): () => void {
    const listeners = this.listeners.get(event) ?? new Set();
    listeners.add(listener as Listener<Events[keyof Events]>);
    this.listeners.set(event, listeners);
    return () => listeners.delete(listener as Listener<Events[keyof Events]>);
  }

  emit<K extends keyof Events>(event: K, payload: Events[K]): void {
    this.listeners.get(event)?.forEach((listener) => listener(payload));
  }

  clear(): void {
    this.listeners.clear();
  }
}
