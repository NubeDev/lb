// Login: node URL + user + workspace → `POST /login` (dev credential; a password/OIDC lands behind
// the same seam later). Errors surface verbatim — an auth refusal renders as text, never a silent
// empty state.

import React, { useState } from 'react';
import { ActivityIndicator, StyleSheet, Text, TextInput, TouchableOpacity, View } from 'react-native';
import { nodeUrl, setNodeUrl } from '../../lib/node-url.store';
import { gatewayClient } from '../../lib/client';
import { devLogin } from '../../lib/dev-defaults';

export function LoginScreen(): React.JSX.Element {
  const [url, setUrl] = useState(nodeUrl() || devLogin.nodeUrl || 'http://192.168.1.10:8080');
  const [user, setUser] = useState(devLogin.user);
  const [workspace, setWorkspace] = useState(devLogin.workspace);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function submit(): Promise<void> {
    setError('');
    setBusy(true);
    try {
      setNodeUrl(url);
      const client = gatewayClient();
      if (!client) throw new Error('enter the node URL');
      await client.login(user.trim(), workspace.trim());
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // The gateway's "not a member" refusal is opaque out of context. Explain the membership rule
      // (global-identity decision #4) so a dev isn't stuck: this user doesn't own this workspace.
      setError(
        /not a member/i.test(msg)
          ? `"${user.trim()}" isn't a member of workspace "${workspace.trim()}". ` +
              `Someone else bootstrapped it — use a fresh workspace name (the first login owns it) ` +
              `or sign in as its admin.`
          : msg,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <View style={styles.screen}>
      <Text style={styles.title}>Lazybones</Text>
      <TextInput style={styles.input} value={url} onChangeText={setUrl} placeholder="Node URL" autoCapitalize="none" autoCorrect={false} />
      <TextInput style={styles.input} value={user} onChangeText={setUser} placeholder="User" autoCapitalize="none" autoCorrect={false} />
      <TextInput style={styles.input} value={workspace} onChangeText={setWorkspace} placeholder="Workspace" autoCapitalize="none" autoCorrect={false} />
      {error !== '' && <Text style={styles.error}>{error}</Text>}
      <TouchableOpacity style={styles.button} onPress={() => void submit()} disabled={busy}>
        {busy ? <ActivityIndicator color="#fff" /> : <Text style={styles.buttonText}>Sign in</Text>}
      </TouchableOpacity>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, justifyContent: 'center', padding: 24, gap: 12 },
  title: { fontSize: 28, fontWeight: '700', textAlign: 'center', marginBottom: 12 },
  input: { borderWidth: 1, borderColor: '#ccc', borderRadius: 8, padding: 12 },
  error: { color: '#c00' },
  button: { backgroundColor: '#111', borderRadius: 8, padding: 14, alignItems: 'center' },
  buttonText: { color: '#fff', fontWeight: '600' },
});
