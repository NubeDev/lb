// The message composer — input + send. Presentation only; posting lives in useChannel.

import React, { useState } from 'react';
import { StyleSheet, TextInput, TouchableOpacity, Text, View } from 'react-native';

export function MessageComposer({ onSend }: { onSend: (body: string) => void }): React.JSX.Element {
  const [draft, setDraft] = useState('');

  function send(): void {
    const body = draft.trim();
    if (!body) return;
    onSend(body);
    setDraft('');
  }

  return (
    <View style={styles.composer}>
      <TextInput
        style={styles.input}
        value={draft}
        onChangeText={setDraft}
        placeholder="Message"
        onSubmitEditing={send}
      />
      <TouchableOpacity style={styles.button} onPress={send}>
        <Text style={styles.buttonText}>Send</Text>
      </TouchableOpacity>
    </View>
  );
}

const styles = StyleSheet.create({
  composer: { flexDirection: 'row', gap: 8, paddingTop: 8 },
  input: { flex: 1, borderWidth: 1, borderColor: '#ccc', borderRadius: 8, padding: 10 },
  button: { backgroundColor: '#111', borderRadius: 8, paddingHorizontal: 16, justifyContent: 'center' },
  buttonText: { color: '#fff', fontWeight: '600' },
});
