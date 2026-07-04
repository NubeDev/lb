// Login: node URL + user + workspace → `POST /login` (dev credential; a password/OIDC lands behind
// the same seam later). Errors surface verbatim — an auth refusal renders as text, never a silent
// empty state.

import React, { useState } from 'react';
import { ActivityIndicator, Text, TextInput, TouchableOpacity, View } from 'react-native';
import { StyleSheet, useUnistyles } from 'react-native-unistyles';
import { nodeUrl, setNodeUrl } from '../../lib/node-url.store';
import { gatewayClient } from '../../lib/client';
import { devLogin } from '../../lib/dev-defaults';

export function LoginScreen(): React.JSX.Element {
  const { theme } = useUnistyles();
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

  const ph = theme.colors.textFaint;
  return (
    <View style={styles.screen}>
      <View style={styles.brandMark}>
        <Text style={styles.brandGlyph}>◐</Text>
      </View>
      <Text style={styles.title}>Lazybones</Text>
      <Text style={styles.subtitle}>Full control for your workspace</Text>
      <View style={styles.form}>
        <TextInput style={styles.input} value={url} onChangeText={setUrl} placeholder="Node URL" placeholderTextColor={ph} autoCapitalize="none" autoCorrect={false} />
        <TextInput style={styles.input} value={user} onChangeText={setUser} placeholder="User" placeholderTextColor={ph} autoCapitalize="none" autoCorrect={false} />
        <TextInput style={styles.input} value={workspace} onChangeText={setWorkspace} placeholder="Workspace" placeholderTextColor={ph} autoCapitalize="none" autoCorrect={false} />
        {error !== '' && <Text style={styles.error}>{error}</Text>}
        <TouchableOpacity style={styles.button} onPress={() => void submit()} disabled={busy} activeOpacity={0.85}>
          {busy ? <ActivityIndicator color="#0C0E0D" /> : <Text style={styles.buttonText}>Sign in</Text>}
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create((t) => ({
  screen: { flex: 1, justifyContent: 'center', padding: t.space(6), backgroundColor: t.colors.ink },
  brandMark: {
    width: 56,
    height: 56,
    borderRadius: t.radius.md,
    backgroundColor: t.colors.mint,
    alignItems: 'center',
    justifyContent: 'center',
    alignSelf: 'center',
    marginBottom: t.space(4),
  },
  brandGlyph: { color: t.colors.ink, fontSize: 30, fontWeight: t.font.weightBold },
  title: { color: t.colors.text, fontSize: 30, fontWeight: t.font.weightBold, textAlign: 'center', letterSpacing: -0.5 },
  subtitle: { color: t.colors.textDim, fontSize: 15, textAlign: 'center', marginTop: t.space(1.5), marginBottom: t.space(7) },
  form: { gap: t.space(3) },
  input: {
    backgroundColor: t.colors.surface,
    color: t.colors.text,
    borderWidth: 1,
    borderColor: t.colors.line,
    borderRadius: t.radius.md,
    paddingHorizontal: t.space(4),
    paddingVertical: t.space(3.5),
    fontSize: 15,
  },
  error: { color: t.colors.danger, fontSize: 13, lineHeight: 18 },
  button: {
    backgroundColor: t.colors.mint,
    borderRadius: t.radius.md,
    paddingVertical: t.space(4),
    alignItems: 'center',
    marginTop: t.space(2),
  },
  buttonText: { color: t.colors.ink, fontWeight: t.font.weightBold, fontSize: 16 },
}));
