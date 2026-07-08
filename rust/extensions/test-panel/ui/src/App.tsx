import { useEffect, useState } from "react";
import {
  LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid,
} from "recharts";
import { bridge } from "./bridge";

interface QueryResult { columns: string[]; rows: (string|number)[][] }
interface Site { id: string; name: string; meterCount: number }
interface Meter { id: string; name: string; site_id: string; point_id: string; point_name: string }
interface TagRow { meter_id: string; tag: string; kind: string; val: string }
interface Reading { hour: string; avg_kwh: number }

const COLOR = "#8b5cf6";

export function App() {
  const [readings, setReadings] = useState<Reading[]>([]);
  const [sites, setSites] = useState<Site[]>([]);
  const [meters, setMeters] = useState<Meter[]>([]);
  const [tags, setTags] = useState<TagRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string|null>(null);

  useEffect(() => {
    (async () => {
      try {
        const SRC = "demo-buildings";
        const [r, s, m, t] = await Promise.all([
          bridge.call<QueryResult>("federation.query", { source: SRC, sql: "SELECT substr(time,1,13) as hour, ROUND(AVG(value),2) as avg_kwh FROM point_reading WHERE time >= '2026-06-05' AND time < '2026-06-06' AND point_id LIKE '%-kwh' GROUP BY 1 ORDER BY 1" }),
          bridge.call<QueryResult>("federation.query", { source: SRC, sql: "SELECT s.id, s.name, COUNT(DISTINCT m.id) as mc FROM site s LEFT JOIN meter m ON m.site_id=s.id GROUP BY s.id,s.name ORDER BY s.name" }),
          bridge.call<QueryResult>("federation.query", { source: SRC, sql: "SELECT m.id,m.name,m.site_id,p.id as pid,p.name as pn FROM meter m JOIN point p ON p.meter_id=m.id ORDER BY m.id LIMIT 50" }),
          bridge.call<QueryResult>("federation.query", { source: SRC, sql: "SELECT * FROM meter_tag LIMIT 20" }),
        ]);
        setReadings(r.rows.map((row) => ({ hour: String(row[0]).replace("T"," "), avg_kwh: Number(row[1]) })));
        setSites(s.rows.map((row) => ({ id: String(row[0]), name: String(row[1]), meterCount: Number(row[2]) })));
        setMeters(m.rows.map((row) => ({ id: String(row[0]), name: String(row[1]), site_id: String(row[2]), point_id: String(row[3]), point_name: String(row[4]) })));
        setTags(t.rows.map((row) => ({ meter_id: String(row[0]), tag: String(row[1]), kind: String(row[2]), val: String(row[3]??"") })));
      } catch(e) { setError(e instanceof Error ? e.message : String(e)); }
      finally { setLoading(false); }
    })();
  }, []);

  if (loading) return <div style={{padding:24,fontFamily:"system-ui"}}>Loading live data from demo-buildings…</div>;
  if (error) return <div style={{padding:24,color:"#dc2626",fontFamily:"system-ui"}}>Error: {error}</div>;

  return (
    <div style={{padding:24,display:"flex",flexDirection:"column",gap:24,fontFamily:"system-ui,-apple-system,sans-serif",background:"hsl(var(--background,210 20% 98.5%))",color:"hsl(var(--foreground,222 30% 16%))",minHeight:"100%"}}>
      <header>
        <h2 style={{margin:0,fontSize:20,fontWeight:700}}>Site Summary — Live from demo-buildings</h2>
        <p style={{margin:"4px 0 0",fontSize:13,opacity:0.6}}>{sites.length} sites · {meters.length} meter points · {readings.length} hourly readings (2026-06-05)</p>
      </header>
      <section style={{border:"1px solid hsl(var(--border,215 16% 86%))",borderRadius:8,padding:16,background:"hsl(var(--card,0 0% 100%))"}}>
        <h3 style={{margin:"0 0 12px",fontSize:14,fontWeight:600}}>Hourly Energy (kWh) — 2026-06-05</h3>
        <div style={{height:280,width:"100%"}}>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={readings} margin={{top:8,right:16,bottom:8,left:0}}>
              <CartesianGrid strokeDasharray="3 3" stroke="rgba(128,128,128,0.15)" />
              <XAxis dataKey="hour" stroke="rgba(128,128,128,0.6)" tick={{fontSize:11}} tickLine={false} axisLine={{stroke:"rgba(128,128,128,0.2)"}} />
              <YAxis stroke="rgba(128,128,128,0.6)" tick={{fontSize:11}} tickLine={false} axisLine={false} />
              <Tooltip contentStyle={{background:"hsl(var(--card,0 0% 100%))",border:"1px solid hsl(var(--border,215 16% 86%))",borderRadius:6,fontSize:12}} />
              <Line type="monotone" dataKey="avg_kwh" stroke={COLOR} strokeWidth={2} dot={{r:3,fill:COLOR}} activeDot={{r:5}} />
            </LineChart>
          </ResponsiveContainer>
        </div>
      </section>
      <Table title="Sites" cols={["ID","Name","Meters"]} rows={sites.map(s=>[s.id,s.name,String(s.meterCount)])} />
      <Table title="Meters & Points" cols={["Meter ID","Meter Name","Site","Point ID","Point Name"]} rows={meters.map(m=>[m.id,m.name,m.site_id,m.point_id,m.point_name])} />
      <Table title="Meter Tags (sample)" cols={["Meter","Tag","Kind","Value"]} rows={tags.map(t=>[t.meter_id,t.tag,t.kind,t.val])} />
    </div>
  );
}

function Table({title,cols,rows}:{title:string;cols:string[];rows:string[][]}) {
  return (
    <section style={{border:"1px solid hsl(var(--border,215 16% 86%))",borderRadius:8,overflow:"hidden",background:"hsl(var(--card,0 0% 100%))"}}>
      <h3 style={{margin:0,padding:"12px 16px",fontSize:14,fontWeight:600,borderBottom:"1px solid hsl(var(--border,215 16% 86%))"}}>{title} <span style={{fontWeight:400,opacity:0.5}}>({rows.length})</span></h3>
      <div style={{overflowX:"auto"}}>
        <table style={{width:"100%",borderCollapse:"collapse",fontSize:13}}>
          <thead>
            <tr>{cols.map(c=><th key={c} style={{textAlign:"left",padding:"8px 16px",fontWeight:600,fontSize:11,textTransform:"uppercase",opacity:0.5,borderBottom:"1px solid hsl(var(--border,215 16% 86%))",whiteSpace:"nowrap"}}>{c}</th>)}</tr>
          </thead>
          <tbody>
            {rows.map((r,i)=><tr key={i} style={{borderBottom:"1px solid rgba(128,128,128,0.08)"}}>{r.map((c,j)=><td key={j} style={{padding:"8px 16px",whiteSpace:"nowrap"}}>{c}</td>)}</tr>)}
          </tbody>
        </table>
      </div>
    </section>
  );
}
